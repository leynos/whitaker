# Implement `whitaker check` as the default linting path

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: COMPLETE (2026-11-18). The user accepted the plan, including decisions
D-1 and D-2, on 2026-11-17, and then instructed implementation to begin.
`EP-M0` to `EP-M3` are complete, and roadmap item 3.5.2 is marked done in
`docs/roadmap.md`. Every gate `EP-M3` names has been run on the final tree; the
evidence recorded under `EP-M3` is as of that milestone, and the later runs
recorded under `Progress` are the final-tree figures.

Roadmap item: 3.5.2 in `docs/roadmap.md`.

## Purpose / big picture

Today a Whitaker user has to provision first and lint second:

```console
cargo install whitaker
whitaker install
whitaker --all -- -p my-crate --all-targets
```

The last line is not a Whitaker command at all. The root `whitaker` binary does
not recognize `--all`, so it forwards every argument verbatim to
`cargo dylint`. This forwarding behaviour is requirement `CLI-REQ-FWD`,
recorded in `docs/adrs/adr-0012-whitaker-cli-boundary.md`. If nothing has been
installed, the user gets Dylint's own error and no Whitaker guidance.

After this change, the documented path is:

```console
cargo install whitaker
whitaker check
whitaker check -- -p my-crate --all-targets
```

`whitaker check` notices when the installation is missing or broken, repairs it
once, and then runs the Whitaker core lints. Every argument after `--` is
passed unchanged to `cargo check`. Continuous integration (CI) can opt out of
repair with `whitaker check --no-install`. A missing installation then becomes
a distinct, documented failure instead of a download in the middle of a build.

The exit status becomes a contract that automation can rely on:

- When Dylint ran, `whitaker check` exits with Dylint's status unchanged, so a
  lint failure looks exactly as it does with `cargo dylint` today.
- When Whitaker could not get as far as running Dylint, it exits with a
  reserved operational code: `4` when the installation needs provisioning, or
  `5` when the host has a fault. It also prints a stable reason key on standard
  error, such as `whitaker: error[provision.refused]`. An operational failure
  is never `0` or `1`, so it cannot be mistaken for a clean run or for lint
  findings.

Success is observable in four ways:

- In a fresh home directory, `whitaker check` repairs the installation, runs
  the lints, and exits with Dylint's own status.
- A second run performs no installation work.
- With `--no-install` on an unprovisioned machine, it exits `4` and advises
  running `whitaker install`.
- On a provisioned machine, `whitaker check -- --message-format=json` writes
  only Cargo's JSON to standard output.

## Definitions

These terms are defined here so that no prior knowledge is assumed.

**Dylint.** A tool that runs custom Rust lints compiled as dynamic libraries.
`cargo-dylint` is its Cargo subcommand. `dylint-link` is the linker wrapper it
uses when building lint libraries from source. Whitaker's lints are Dylint
lints.

**Lint bundle.** A compiled Whitaker lint library, such as
`libwhitaker_suite@nightly-2026-05-28.so`, staged in a directory that Dylint
reads through `DYLINT_LIBRARY_PATH`. The file name encodes the nightly
toolchain that built it.

**Bootstrap.** The installed launcher whose metadata lives at
`<whitaker_data>/whitaker-bootstrap/bootstrap.json`. It runs `cargo dylint`
with Whitaker's verified Clippy snapshot, a managed driver slot, and a lock
that serializes the first ("cold") driver build. Its code lives in
`installer/src/bootstrap/`. It is already split into a side-effect-free query,
`prepare`, which checks the metadata, the library directory, the Clippy
snapshot and the recorded `cargo-dylint` version, and a command, `execute`,
which runs the child.

**Preflight.** This plan's name for the bootstrap's `prepare` query once it is
made public. A successful preflight returns an opaque `PreparedRun`. A failed
one returns a typed `BootstrapError` that says what is wrong.

**Lazy repair.** Running the installation flow from inside `check`, once, when
preflight reports a repairable problem, and then running preflight again before
linting.

**User-managed library path.** A non-empty `DYLINT_LIBRARY_PATH` set by the
user. It is authoritative, and Whitaker never repairs or replaces it.

**Operational error.** A failure that stops Whitaker before Dylint produces a
lint result, for example an invalid configuration, a refused or failed repair,
an unreadable data directory, or a `cargo dylint` process that cannot start.

**Forwarded Cargo arguments.** The arguments after the first `--` on the
`whitaker check` command line. Dylint passes them on to `cargo check`.

**Port / adapter.** A port is a trait owned by the command-policy layer that
describes something the policy needs from the outside world. An adapter
(`src/adapters/`) implements it against the real system. Tests substitute fakes.

**Kani.** A bounded model checker for Rust. A `#[kani::proof]` harness explores
every execution of real Rust code within stated bounds. Over a finite domain,
that exploration is exhaustive.

**Verus.** A deductive verifier. In this repository, files under `verus/` model
the implementation and prove properties for all inputs. Kani then checks the
real code against the same property. See `docs/developers-guide.md`, "Verus
scope and trust boundary".

## Constraints

These are hard invariants. Violating one requires escalation, not a workaround.

1. `CLI-REQ-FWD` remains in force. Every invocation that is not `install`,
   `ls`, `check`, root help, or root version is still forwarded verbatim to
   `cargo dylint`, with its status preserved. `make lint` and CI depend on
   `whitaker --all ...`. Forwarding is removed only in roadmap item 3.9.3.
2. The 3.5.1 exit policy for `install`, `ls`, display requests, usage errors
   and legacy forwarding does not change (`EP-INV-EXIT` in the 3.5.1 plan).
   Codes `3` to `9` acquire meanings that no command may contradict, but only
   `check` uses them in this item (decision D-5).
3. The command layer decides whether and when to repair. The installer library
   decides how (ADR 0012). `check` never reimplements an installation step.
   Readiness is judged by the installer's own bootstrap preflight. The plan
   does not create a second readiness model.
4. Standard output belongs to the Dylint child during `check`. Whitaker writes
   all progress, repair output and diagnostics to standard error. That includes
   the installer's `whitaker-installer: suite-source=...` marker, which
   `whitaker install` writes to standard output.
5. A user-managed library path stays authoritative. `check` never repairs
   anything while one is set. It reports what is wrong and refuses.
6. Domain and reusable library crates do not depend on `ortho_config`
   (ADR 0008).
7. No new external dependencies. The root package already has `rstest`,
   `rstest-bdd`, `googletest`, `pretty_assertions`, `insta`, `proptest`,
   `tempfile` and `cap-std`. The installer already depends on `fs2` for file
   locks.
8. No file may exceed 400 lines. `src/adapters/mod.rs` (360 lines),
   `src/adapters/library_path/mod.rs` (399) and `src/adapters/telemetry/mod.rs`
   (388) are near the limit. New code goes in new modules.
9. Environment access follows `AGENTS.md`, "Inject the environment instead".
   - Production code reads `CI` and `DYLINT_LIBRARY_PATH` only at the
     composition root (`SystemServices`). It passes the values inward as
     injected data or through a narrow closure seam. Policy code never calls
     `std::env::var` or `std::env::var_os`.
   - In-process tests that need environment values use the
     `whitaker_common::test_support` helpers (`with_env_var`,
     `with_env_var_removed`, `with_env_vars`). Whitaker does not depend on
     `mockable`.
   - End-to-end tests set the environment on the spawned child `Command`
     only. Process-wide locks around environment mutation are not an accepted
     substitute.
10. Rule codes, selectors, exit codes, reason keys and JSON keys are stable and
    never translated. Human-facing prose uses the existing localization path
    with an `en-GB` fallback.
11. Final proof files contain no `assume`, `admit`, `external_body` or
    equivalent trusted shortcut.
12. Clippy lints denied in `Cargo.toml` apply. In particular,
    `integer_division_remainder_used` forbids `%`, which matters for Kani tag
    generators.
13. Every `assert!`, `assert_eq!` and `assert_ne!` in new tests carries a
    diagnostic message that states the failed expectation (`AGENTS.md`, Rust
    guidance). `googletest` matchers and `pretty_assertions` are used where
    they give richer failure output.

## Tolerances (exception triggers)

ExecPlan files are excluded from every count below, both file counts and line
counts, including when this plan is edited in the same commit.

- Scope: code and test work (`EP-M0` to `EP-M2`) may touch at most 30
  non-ExecPlan files and 2,500 net lines. Documentation (`EP-M3`) has a
  separate budget of 12 files and 800 net lines. If either budget is exceeded,
  stop and escalate.
- Installer interface: in scope are promoting `bootstrap::prepare`/`execute`
  to public `preflight`/`run_prepared`, making `PreparedRun` public with
  private fields, and adding an installation-lock function. Changing the
  signature of an existing public installer item (`InstallArgs`,
  `run_install_for_whitaker`, `run_with_options`) requires escalation.
- Surface: adding a `check` flag that is not in the design's
  §`whitaker check` help surface requires escalation. Examples are
  `--manifest-path` and `--fix`.
- Dylint semantics: if the `EP-M0` prototype falsifies H-1, H-3 or H-7, stop
  and revise the invocation design before `EP-M1`.
- Iterations: if a focused test still fails after three fix attempts, stop and
  record the failure in `Surprises & discoveries`.
- Verification: if a Verus obligation does not verify after two hours of
  proof work, or `./scripts/run-verus.sh cli` exceeds 600 seconds on two
  consecutive runs, stop and escalate. Do not weaken a theorem.
- Ambiguity: if the design document and the roadmap disagree about behaviour
  that a test would observe, stop and present the options.

## Risks

- **Roadmap prerequisites are not merged.**
  - Severity: high. Likelihood: certain.
  - Item 3.5.2 lists 3.6.2 (selector precedence), 3.6.4 (shared presentation
    flags) and 3.7.1 (`whitaker install` with `--offline`,
    `--build-from-source` and `--toolchain`) as requirements. None is ticked,
    3.6.1 (rule codes) is also open, and no open pull request covers any of
    them as of 2026-11-11.
  - Mitigation: decision D-1 splits the item so that the behaviours that do
    not depend on those prerequisites ship now. This needs explicit approval.

- **`cargo dylint` exit `1` is overloaded.**
  - Severity: medium. Likelihood: certain.
  - `cargo dylint` exits `1` for a lint denial, a compile error in the user's
    code, a forwarded Cargo argument it rejects, and any internal Dylint
    failure. Its `main` returns `anyhow::Result`. It exits `2` for its own
    usage errors and `101` only if it panics. Cargo's own `101` never comes
    through.
  - Mitigation: preflight removes the installation-related causes before
    Dylint starts. ADR 0017 defines a passed-through `1` as "Dylint or Cargo
    reported failure", not "lint findings only".

- **A stale bundle passes preflight after a CLI upgrade.**
  - Severity: medium. Likelihood: medium.
  - The bootstrap records the toolchain, the Dylint version and the snapshot,
    but not the Whitaker version that built the bundle. A new CLI on the same
    nightly can therefore run old lints silently.
  - Mitigation: this is a known gap owned by 3.7.2 (source provenance) and
    3.7.3 (bundle manifests). `check -v` prints the bundle's recorded
    toolchain and Dylint version so the gap is visible. The users' guide
    records it.

- **Concurrent lazy repairs race.**
  - Severity: medium. Likelihood: plausible in CI matrices that share a home
    directory, in parallel pre-commit hooks, and in IDE-plus-terminal use.
  - The installer locks only the installation record, the metrics file, the
    snapshot install and the driver slot. Staging and tool installs are not
    serialized.
  - Mitigation: repair runs under a new exclusive installation lock. After
    acquiring the lock, the installer runs preflight again and skips the
    install if another process has already completed it.

- **Selection changes may not invalidate Cargo's check cache.**
  - Severity: high for phase two. Likelihood: unknown.
  - If Dylint does not track `DYLINT_RUSTFLAGS` as a rebuild input, changing
    `--select` on a warm cache re-checks nothing and reports stale results.
  - Mitigation: `EP-M0` hypothesis H-7 settles this before the selection
    design is fixed.

- **The `--manifest-path` caveat.**
  - Severity: low. Likelihood: medium.
  - Dylint resolves its metadata and target directory from the current
    directory unless `--manifest-path` comes before its `--`. That means
    `whitaker check -- --manifest-path x/Cargo.toml` run outside the
    workspace can fail with a passed-through `1`.
  - Mitigation: decision D-6 documents the limitation. Adding a
    `--manifest-path` flag is a design change outside this item.

- **Claiming the word `check` changes behaviour.**
  - Severity: low. Likelihood: low.
  - Today `whitaker check` forwards to `cargo dylint check`, which
    `cargo dylint` rejects, so no working workflow is lost.
  - Mitigation: record the change in the users' guide.

## Progress

- [x] (2026-11-11) Branch renamed to
  `3-5-2-whitaker-check-as-the-default-linting-path`.
- [x] (2026-11-11) Wyvern reconnaissance: the root CLI, the installer, the
  configuration package, and the verification infrastructure.
- [x] (2026-11-11) External research on `cargo dylint` exit statuses and
  library selection.
- [x] (2026-11-11) First draft written and committed.
- [x] (2026-11-11) Expert design review run with three independent reviewers
  (structure and alternatives, contracts and scaling, failure modes and
  viability). Their findings are recorded under `Decision log`, D-7.
- [x] (2026-11-11) Plan revised in response to the review.
- [x] (2026-11-16) Branch rebased onto `origin/main` `40a85aa5`, and the
  plan reconciled with 14 new commits on `main`: the ADR number moved to 0017,
  the prover installation steps were added, the experimental-lint model was
  updated, and line references were refreshed.
- [x] (2026-11-17) Plan approved by the user, including D-1 and D-2.
- [x] (2026-11-17) Rebased onto `origin/main` `45a436bc` (5 new commits) and
  reconciled. ADR 0016 was taken on `main`, so the exit contract became ADR
  0017. The installer's `prepare`/`execute` split and the bootstrap driver-slot
  rework survived the rebase unchanged in shape, so D-3 holds.
- [x] (2026-11-17) `EP-M0` prototype: Dylint invocation semantics settled.
  H-1 to H-5 and H-7 are not falsified; H-6 is deferred to `EP-M2`, where the
  lock function exists to test.
- [x] (2026-11-17) `EP-M1` provisioned-path `check` with the exit contract.
  `check` runs the suite through the promoted bootstrap preflight, returns
  Dylint's status unchanged, refuses a repairable installation with exit `4`,
  and reports `host` and `exec` faults with exit `5`. ADR 0017 is drafted and
  indexed in `docs/contents.md`. The deviations from the plan are recorded in
  D-9, and the one case the plan's method could not construct is recorded under
  `Surprises & discoveries`.
- [x] (2026-11-18) `EP-M2` lazy repair, `--no-install` and the installation
  lock. A plain `whitaker check` repairs under `with_install_lock`, preflights
  again and lints; `--no-install` and a user-managed path refuse with exit `4`;
  standard output stays pure and `whitaker.check.repairs` is emitted. The
  deviations are recorded in D-10 and the evidence in
  `Outcomes & retrospective`.
- [x] (2026-11-18) CodeRabbit review reconciliation, first batch. The review
  raised 104 findings on PR #603; the ones that change behaviour are actioned
  here. Readiness now yields a value that cannot be forged: a user-managed path
  is inspected once, by `DirectLibraryPath::new`, and both the readiness check
  and the later run share that inspection, so no prepared value can name a path
  Dylint would exit `1` for. The `check` diagnostics are localizable through
  `ortho_config`'s `Localizer`, with the bracketed reason key kept verbatim
  because automation reads it. A lazy repair replays the suite pin the
  installation recorded, and every install writer now takes the same
  installation lock.
- [x] (2026-11-18) The shared bootstrap fixture stages the suite library into
  the recorded library directory. `check` refuses a recorded installation whose
  directory does not hold `whitaker_suite`, so the six end-to-end and
  behavioural tests that assert pass-through were red until the fixture matched
  a real installation. The user-managed feature step stages one too, for the
  same reason.
- [x] (2026-11-18) `EP-M3` documentation, ADR 0017 and roadmap update. 3.5.2
  marked done.
- [x] (2026-11-19) CodeRabbit review reconciliation, second batch. The same
  PR #603 review was re-triaged finding by finding. Several major findings were
  already satisfied by the first batch and are dispositioned rather than
  re-implemented: the locked preflight returns host faults instead of
  repairing, snapshot I/O is a `host.io` fault, metadata read failures stay on
  the provisioning path, the one-repair recovery covers a run-time
  `Provisioning` fault, and explicit installs take the installation lock via
  `run_install_locked` while the repair calls the `_under_lock` variant.
  - The user-managed suite rule is now stated as *exactly one*: a path that
    holds none, or more than one, is refused with exit `4` and
    `provision.user-path`, and Dylint never starts. The BDD feature covers
    both refusals, and the `CHK-INV-POLICY` obligation states the same rule.
  - A recorded bootstrap now checks the user's override for the suite as
    well, closing the gap where an empty `DYLINT_LIBRARY_PATH` reached the
    child and exited `1`.
  - `CheckRequest` carries the root diagnostic settings and the localizer, so
    a lazy repair inherits `-q`/`-v` and a refusal can be translated without
    localizing its bracketed key.
  - The diagnostic writer flushes the stream it wrote to; the built-driver
    notice is reported without replacing a completed run's status.
  - Tests were split to stay under the 400-line rule
    (`src/cli/check/tests/`, `tests/behaviour_check_repair/`,
    `tests/e2e_check_support/`), and the Kani witnesses moved to
    `src/cli/check/flow/witnesses.rs` with shared samplers in
    `src/cli/check/flow/samplers.rs`.
  - D-10 gained the ambiguity rule as an explicit refinement; nothing in D-1
    to D-9 changed. Lesson: a fixture that stages only a directory named like
    the suite is not evidence, because the readiness check now requires a
    file; and a doctest that shows a refusal must opt out of repair, or an
    unprovisioned machine repairs instead of refusing.
- [x] (2026-11-19) Final gate round. Three defects the gates found in the
  second batch were fixed rather than argued away: the shared bootstrap
  fixtures returned `Result` so `clippy::expect_used` stayed strict
  (`installer/src/bootstrap/run_api_tests.rs`), with the assertions moved into
  helpers because `panic_in_result_fn` forbids them in a `Result` function;
  `installer/src/orchestration/mod.rs` had grown to 413 lines and its two
  announcement helpers moved to `installer/src/orchestration/announce.rs` (369
  lines remain); and the re-blessed UI fixture
  `installer/tests/ui/legacy_installer_error_match.stderr` carried a stray
  leading space before the two-digit line number, which only the byte-exact
  trybuild comparison exposes. Helper doc summaries that began with a noun
  phrase were rewritten to open with a verb.
- [x] (2026-11-19) Proof gates on the final tree. `make verus` was already
  green, but the Kani group was red, and the plan's recorded 8/12 counts
  predated the review batch, which had grown the set to 9 properties and 13
  witnesses. The red harness was
  `verify_check_flow_runs_only_fresh_ready_preflight`, whose third assertion
  required the last run to consume the final readiness check. That is not a
  property the flow has, and it never had it: a run-time provisioning failure
  spends the repair, the repair succeeds, and the second check can then say the
  installation is still repairable, so the flow ends `NotConverged` with the
  most recent check *after* the run. The assertion now compares the consumed
  index against the preflight count recorded when that run started, which is
  the freshness claim the flow really makes and still catches a stale prepared
  value. Lesson: an exhaustive harness is only as good as its assertion; this
  one was unreachable-by-construction for the flow's real branch set, and only
  a final-tree run over the enlarged harness set exposed it.
- [x] (2026-11-19) CodeScene delta on the final tree. `cs delta` reported two
  new issues, and both were refactored rather than suppressed. The message
  tests repeated the render-and-decode steps four times, so `render_with` now
  returns the text a user would see and each test states only its own
  assertion. The Kani port took its script as a list of primitives (`[u8; 2]`,
  `u8`, `bool`), which tripped the primitive-obsession rule at 50%; `Script`,
  `Answer`, `RepairAnswer` and `RunAnswer` name what a scripted port is given,
  the samplers map a symbolic `u8` onto those variants, and every helper now
  takes the port, the script, or the policy instead of loose numbers. The
  message module and both flow proof modules score 10.00.
- [x] (2026-11-19) The review's pre-merge testing row, closed by tests rather
  than by argument. The row named four behaviours introduced by `check` whose
  removal no test would catch: the in-lock readiness re-check, the routing of
  the installer's markers to the diagnostic stream, the CI advice on a repair,
  and the run and repair fault tables. `repair_under_lock` now drives its two
  steps through a `RepairSteps` seam and takes its CI flag and streams as
  arguments, so `repair_with` can be driven without a real installation:
  `tests/repair_flow.rs` pins the ready, host-fault, pinned-repair, CI,
  failed-install, marker-routing and unwritable-notice paths, and
  `cli/check/tests/faults.rs` pins every arm of both fault tables. Removing an
  arm, inverting a branch, or sending a marker to the diagnostics stream fails
  one of these tests. The `CI` read now sits beside the adapter's other
  environment facts instead of inside the notice writer.
- [x] (2026-11-20) Final review round, driven by the two ExecPlan assessments.
  Both correctness defects the completeness assessment found are fixed with
  regression tests: readiness may approve an installation holding one suite per
  toolchain, and the run's own re-check now asks readiness's question rather
  than the stricter exactly-one rule (`prepared_suite_available`); and a repair
  whose notice cannot be written is `exec.output` (`CheckRepairError::Output`),
  not a host fault. `CHK-INV-ARGV` is now an end-to-end property over 4,096
  cases with coverage thresholds; `CHK-LEM-ARGV` is stated and proved for any
  separator-free `W`, not the two-option prefix; the exit and flow witness sets
  cover every outcome class the flow can reach; preflight, decision and repair
  events are captured through a subscriber; and the exit-contract module doc
  states the pass-through overlap instead of implying total disjointness. Both
  negative controls were run and reverted. Counts, scope escalation and the
  dispositions of every assessment finding are in D-11.
- [x] (2026-11-20) Pre-merge documentation rows, closed by writing the missing
  documents. CodeRabbit's User-Facing Documentation check failed because
  `docs/migrations/0.4.0.md` did not exist and the README did not signpost
  `whitaker check`; both are now written, the migration note is indexed in
  `docs/contents.md` above the 0.2.8 entry, and `docs/users-guide.md` links it
  first in "Upgrading library consumers". Its Developer Documentation check
  failed because the bootstrap boundary published `PreparedRun`, `preflight` and
  `run_prepared` without describing them; the section now lists every public
  entry point, what `preflight` verifies, and that `run_prepared` accepts
  nothing but a successful `PreparedRun`.
- [x] (2026-11-20) CodeScene delta on the final tree, second round. `cs delta`
  reported eight similar exit witnesses and the two `ProbePort` constructors as
  duplication. The eight witnesses became one `exit_witness!` invocation each —
  same harness names, same witness and failure strings, because
  `scripts/run-kani.sh` matches those strings — and both constructors now
  delegate to `ProbePort::from_plan`. `cs delta` then reported no issues in the
  checked changes, and both files are shorter (`exit.rs` 363 lines,
  `verification.rs` 293).
- [x] (2026-11-20) Final-tree gates. The refactors touch only `#[cfg(kani)]`
  modules, so the run covered the gates that read them plus every Markdown gate
  the new documents reach: `make check-fmt`, `make markdownlint`,
  `make spelling` and `./scripts/run-kani.sh whitaker-cli` all exited `0`. Kani
  reported 9 verified harnesses and 20 expected counterexamples, with no
  witness or message drift; the other seven gates were green on the same tree
  before the documentation and `#[cfg(kani)]`-only increments.
- [x] (2026-11-20) Second completeness assessment
  (`EP352-COMPLETE-20261120T060853Z`): all five findings actioned. A recorded
  library directory is held to Dylint's exactly-one rule, so a directory with
  two suites is refused as `provision.refused` rather than reaching Dylint and
  returning its `1` as a lint result; the user-managed route reports
  `host.data-dir` when the data directory is unknown; the CI decision and the
  repair streams are seams the production code and the tests share; H-6 has its
  two-process test in `tests/e2e_check_lock.rs`
  (`cargo nextest run -p whitaker --test e2e_check_lock`: 1 passed in 3.6 s);
  and five negative controls (argument units and property, Io classification,
  Verus separator, Kani at-most-once) have transcripts and results. D-12
  records every disposition.
- [x] (2026-11-20) Proofs assessment (`EP352-PROOFS-20261120T060853Z`): four
  concerns dispositioned at head `26524e55`. Three controls the plan required
  were run and reverted — Kani routing (`1 of 362 failed`), Verus routing
  (`6 verified, 2 errors`) and Kani exit mapping (`1 of 46 failed`) — and the
  plan now bounds the flow, argument, exit and routing claims to what their
  harnesses establish. D-13 records every disposition and transcript.
- [x] (2026-11-20) Third assessment round
  (`EP352-COMPLETE-20261120T060853Z` and `EP352-PROOFS-20261120T060853Z` at head
  `175c7e04`): both findings actioned. The prepared value carries the route
  that approved it, so the run re-check keeps the user-path reason and a
  user-path run fault is refused under `provision.user-path` with no repair
  spent (`F7`). The production marker stream is observed from a child process at
  `repair_production` itself. The plan's two-part `F3` control was run and
  fails as specified, the Io obligation now names the classifier tables, and
  the control transcripts are inline. The `check` port moved out of the ports
  module into `src/ports/check.rs`, re-exported through `whitaker::ports`
  unchanged, because the route field took the module past the 400-line file
  limit; the parent became `src/ports/mod.rs` because the workspace denies
  `clippy::self_named_module_files`. Step 9 of this plan authorizes exactly
  that split. D-14 records every disposition.
- [x] (2026-11-20) CodeRabbit disposition pass: the 20 unresolved threads were
  verified against head `3acce0f5` and the still-valid residuals repaired. The
  Kani samplers now derive `kani::Arbitrary` on both enums, so no wildcard arm
  can stand in for an unsampled variant; the adapter's `check_run` doc matches
  the precedence it implements, and the two preflight functions carry
  `# Errors`; 16 adapter tests gained `///` summaries; the staging-only fixture
  helpers moved to `tests/behaviour_whitaker/root_cli.rs`, so the shared
  fixture needs no module-wide `dead_code` expectation; and two regressions
  were added — a completed run keeps its status through a refused built-driver
  notice, and a locked preflight I/O fault refuses without installing. The
  `windows-compat` failure on `3acce0f5` was a `#[cfg(unix)]` import used only
  by unix tests; `cargo fmt --all` and a `-D warnings`
  `cargo check -p whitaker --lib --tests` pass on the repaired tree.
- [x] (2026-11-20) Fourth assessment round
  (`EP352-COMPLETE-20261120T060853Z` and `EP352-PROOFS-20261120T060853Z` at head
  `3acce0f5`): three code findings and one evidence finding actioned. The
  direct user-managed route re-checks its suite before Dylint starts, so a path
  that changed after readiness is refused under `provision.user-path`; a
  run-time provisioning fault under `--no-install` is refused under its
  readiness key instead of being reported as non-convergence; and the earlier
  control records now carry their mutation, command and decisive lines, re-run
  on this tree together with the argument controls. A CodeScene duplication
  finding on the round-four tree merged those two refusal tests into one
  `rstest` table, which scores 10.00 with no warning. D-15 records every
  disposition.
- [x] (2026-11-20) `windows-compat` repair. The gated imports cleared the
  failure the round-four assessment named, but the job then failed on
  `adapters::check::tests::repair_flow::
  a_locked_preflight_host_fault_is_reported_without_installing`:
  a file standing where the data directory belongs is `ENOTDIR` on Unix, while
  Windows reports the same path as not found, which is the answer a missing
  record also gives. Production readiness therefore answered "repairable" and
  the test saw `Ok(())`. The test and its `RealReadiness` import are now
  `#[cfg(unix)]`, with the platform reason in the test's doc, and the policy
  stays covered on every host by `a_host_fault_is_reported_without_installing`.
- [x] (2026-11-20) Fifth assessment round
  (`EP352-COMPLETE-20261120T060853Z` and `EP352-PROOFS-20261120T060853Z` at head
  `c17c4264`): three findings, all actioned. The `windows-compat` test step
  still failed, now because the round-four gating left `RealReadiness` without
  a Windows consumer; the fixture, its implementations and its `SystemSteps`
  import are `#[cfg(unix)]` too. The direct route gained the second-suite case
  and a recording `cargo` stub: `run_direct_with_cargo` names the program, so
  `a_user_path_that_stopped_naming_one_suite_is_refused_before_cargo_starts`
  observes that neither changed shape starts a child instead of inferring it
  from the returned error. The proofs reply showed the user-path control mutated
  `classified`, which `ran`'s refusal arm now bypasses; both records are
  marked historical and the control is re-run against `ran`. D-16 records all
  three dispositions.
- [x] (2026-11-20) Notice precedence reaches the run boundary
  (thread reply of `EP352-COMPLETE-20261120T060853Z` on
  `docs/whitaker-cli-design.md`): the completed-status rule was pinned only on
  `completed_status`, so the caller's answer itself was untested.
  `run_bootstrapped` now takes its inputs as a `BootstrappedRun` and the run to
  start as a closure; production passes `run_prepared`, and the boundary test
  presents a completed `1` with a refused notice and receives `1`. The run path
  moved to `src/adapters/check/run.rs`, because the adapter reached 429 lines
  and constraint 8 sends new code to new modules. D-17 records both, and a
  control shows the helper test and the boundary test fail together when the
  precedence is inverted.
- [x] (2026-11-21) Composition-root `PATH` and boundary telemetry
  (revision-10 reply of `EP352-COMPLETE-20261120T060853Z`): the readiness
  helper still read `PATH` itself, and the readiness port still recorded its
  own telemetry. `PATH` is now read once in `SystemServices::preflight` and
  `SystemServices::repair` and handed down as a value, including through the
  repair seam's `HostEnv`; recording moved to `RecordedPorts`, a wrapper over
  `CheckPorts` built at the command boundary, so the flow stays free of
  adapters and the post-repair question is still recorded. D-18 records the
  decision, four alternatives and both controls.
- [x] (2026-11-21) One environment snapshot per invocation
  (revision-11 reply of `EP352-COMPLETE-20261120T060853Z`): per-call reads left
  a repair flow reading `PATH` three times, against the module's own "once per
  invocation" claim. `SystemServices` now snapshots `PATH`,
  `DYLINT_LIBRARY_PATH` and `CI` in `new`, in its own module
  `src/adapters/services.rs`; `active_library_path` takes the explicit setting
  as a parameter, so nothing below the composition root reads the environment.
  D-18's round-seven correction records the decision and the two new
  `library_path` tests.
- [x] (2026-11-21) Disposition the published `SystemServices` break
  (revision-12 reply of `EP352-COMPLETE-20261120T060853Z`): the snapshot
  changes the shape of a type the published 0.3.0 crate exports, so
  `docs/migrations/0.4.0.md` now carries the migration and the plan records the
  compatibility decision. The same reply's Windows `PATH` point is recorded as
  a verification limit rather than overstated coverage. D-18's round-eight
  corrections hold both.
- [x] (2026-11-21) Final-tree gate evidence, recorded here because `EP-M3`'s
  figures are historical. All ten gates ran sequentially on the tree of
  `b7b007a1` (`695169a84825dea56c9652a2358188f28d6bbca9`): `make check-fmt`
  left 131 files unchanged; `make markdownlint` reported `Summary: 0 error(s)`
  over 131 files; `make nixie` validated every diagram;
  `make test-workflow-contracts` passed 1023 tests; `make lint` ran interrogate
  at 100%, rustdoc, Clippy with `-D warnings` and the Whitaker Dylint suite
  clean; `make typecheck` exited `0`; `NEXTEST_PROFILE=ci make test` ran 2879
  tests with 2879 passed and 10 skipped, then its follow-on passes at 134, 210
  with 1 skipped, and 50; `make test-doc` passed 463 doctests; `make kani`
  verified 27 harnesses, had 20 expected counterexamples validated by
  `run_expected_counterexample`, and reported the 3 documented inconclusive
  decomposition harnesses; `make verus` verified 70 with 0 errors.
- [x] (2026-11-21) Formal review `6002879947` (revision-13 tree): eleven inline
  findings and one outside-diff finding are repaired.
  - `src/library_path.rs` now models the three rules Dylint 6.3.0 applies to a
    library path list: every entry must be absolute and name an existing
    directory, discovery is by file name, and distinct discovered libraries are
    counted once. A single suite-named candidate must resolve to a regular
    file. The refusals are `UserPathUnusable` (user path) and
    `RecordedPathUnusable` (record), classified as a user-path repair and a
    metadata repair respectively. The earlier tolerance of a missing directory
    is reversed: Dylint rejects the whole list, so skipping it would pass on a
    list Dylint refuses.
  - `BootstrapMetadata::read` reads bytes and parses from the slice, so damaged
    content — including invalid UTF-8 — is malformed metadata rather than an
    I/O fault; `a_record_that_is_not_utf8_is_malformed` pins it.
  - `run_install_for_whitaker` delegates to `run_install_locked`, so
    `whitaker install --dry-run` takes no installation lock. The root
    behaviour scenario now runs against a data home that has never been
    installed into and asserts `no installation lock is created`.
  - `tests/e2e_check_lock.rs` replaces the runtime threshold with a
    handshake: the check must hold an open descriptor on `install.lock`
    (observed through `/proc/<pid>/fd`) before the holder publishes the
    record, with a bounded timeout for failed coordination. The target is
    Linux-gated because that is the only hosted lane running this suite.
  - `docs/migrations/0.4.0.md`, `docs/users-guide.md`,
    `docs/whitaker-cli-design.md` and `docs/v0-3-0-migration-guide.md` carry the
    key qualification, the removal of the offline claim, the repair
    prohibition, and the `InstallLock` correction. `EP-M3`'s gate figures are
    marked historical.

## Surprises & discoveries

- **None of the prerequisites for 3.5.2 is complete.**
  - Evidence: `docs/roadmap.md` shows 3.6.1, 3.6.2, 3.6.4 and 3.7.1 unticked,
    and `gh pr list --state open` shows no pull request for them.
  - Impact: drives decision D-1.

- **The root CLI already conflates operational failure with lint failure.**
  - Evidence: `src/cli/exit.rs` maps `Outcome::OperationalFailure` to `1`, the
    same status `cargo dylint` returns for lint failures.
    `cargo-dylint/src/main.rs` returns `dylint::ColorizedResult<()>`.
  - Impact: `check` needs its own exit contract (D-2, ADR 0017).

- **The bootstrap route collapses every typed error into exit `1`.**
  - Evidence: `installer/src/install_flow/bootstrap_step.rs:206-226`
    (`run_bootstrap_with_dirs`) maps every `BootstrapError` to `1`, while the
    public `bootstrap::run_with_options` returns
    `Result<RunOutcome, BootstrapError>`.
  - Impact: `check` must call the typed API, never the `i32` wrapper that
    forwarding uses. Found independently by two reviewers.

- **The bootstrap already contains the readiness query.**
  - Evidence: `installer/src/bootstrap/run.rs:120-140`. `prepare` checks the
    metadata, the library directory, the Clippy snapshot and the
    `cargo-dylint` version, and `BootstrapError` names each failure.
  - Impact: the first draft's separate readiness model would have
    disagreed with it. Readiness is now the bootstrap preflight (D-3).

- **No install-wide lock exists.**
  - Evidence: `fs2` locks appear only in `installation_record/mod.rs:86`,
    `install_metrics.rs:251` and `bootstrap/driver.rs:260-276`.
  - Impact: repair needs a new lock (D-4).

- **`run_install_for_whitaker` cannot repair one component.**
  - Evidence: `orchestration/mod.rs:126-183` (`run_install`) and `:223`
    (`plan_install`). It always installs the snapshot, prepares the workspace
    and the toolchain, and stages a bundle. The only partial switch is
    `skip.skip_deps` (`:144`). It also updates the managed clone unless
    `skip.no_update` is set (`:282`).
  - Impact: repair is coarse and runs with `no_update = true` (D-3).

- **Proof CI does not run on root CLI changes.**
  - Evidence: the `paths` filter in `.github/workflows/proofs.yml` lists no
    `src/**` path. `run_whitaker_cli_harnesses` in `scripts/run-kani.sh`
    (line 232) names each harness explicitly.
    `scripts/run-verus.sh` must list a new file in both `cli` and `all`.
  - Impact: three wiring edits are now explicit plan steps.

- **ADR numbers 0013 to 0016 are taken, so the exit contract is ADR 0017.**
  - Evidence: on 2026-11-17, after fetching every remote branch and scanning
    `docs/adrs` across all of them:
    - `main` holds `adr-0013-node-origin-provenance-classifier.md`,
      `adr-0014-per-lint-experimental-opt-in.md`,
      `adr-0015-brain-trust-lint-driver-interfaces.md` and
      `adr-0016-question-mark-is-a-branch.md`.
    - Every other `refs/remotes/origin/*` branch holds nothing numbered
      0017 or above.
  - History: the first draft used 0014, which collided once `main` advanced;
    revision 2 moved it to 0016, which `main` then took as well. Revision 4
    moves it to 0017. Re-scan every remote branch immediately before writing
    the ADR in `EP-M3`, and renumber again if 0017 has been taken meanwhile.
  - Impact: the exit-contract ADR is `adr-0017`.

- **`main` advanced 14 commits between authorship and the first rebase.** The
  plan was rebased from `d9eb8a09` onto `40a85aa5` on 2026-11-16. The changes
  that affect this plan are:
  - Experimental lints are now enabled per lint and per repository (#625,
    ADR 0014 on `main`).
    - The published suite contains every experimental lint. The driver
      registers a lint's pass only when the consuming repository names it under
      `[whitaker] experimental` in `dylint.toml`.
    - `whitaker_common::experimental::resolve` is the shared validator.
    - There is therefore no separate experimental bundle, and readiness
      (`EP-M1`, `EP-M2`) needs no experimental component.
    - An unknown name in that table is a fatal driver diagnostic, which
      reaches `check` as a passed-through `1`.
    - The design document now names `[lint].experimental_lints` in
      `whitaker.toml`, which no code reads yet, as the CLI-level spelling.
      That belongs to 3.5.4 (see the phase-two obligations).
  - Kani and Verus are now installed through `rust-prover-tools` (#615).
    - `scripts/run-kani.sh` and `scripts/run-verus.sh` no longer install
      anything. They fail with an install hint.
    - `make install-kani` and `make install-verus` are the installation
      steps, and CI uses the shared `install-prover-tools` action.
    - `run-verus.sh` now drives Verus through `prover-tools verus run`, and the
      `all` group gained `verus/node_origin.rs`.
    - `proofs.yml` now watches `tools/kani/**`, `tools/verus/**` and
      `tools/rust-prover-tools/**`, but still not `src/**`.
  - `AGENTS.md` was realigned with the Rust project template (#621). Doctests
    no longer run in `make test`; `make test-doc` runs them.
  - `docs/documentation-style-guide.md` changed (#617).
    - Emphasis now uses underscores.
    - Its Rust example shows `# Parameters`/`# Returns` sections. That
      conflicts with `AGENTS.md`, which forbids that boilerplate. This plan
      follows `AGENTS.md`, the more specific Rust rule.
  - Version 0.3.0 was released (#631). The root `whitaker` package is still
    pre-1.0, so the compatibility analysis is unchanged.
  - The bootstrap driver-slot locking was reworked (`driver.rs`,
    `driver_run.rs`), but `run.rs` (`prepare`) and the signature of
    `driver_run::execute` are unchanged. D-3 still holds.
  - The installer's line references moved. They are corrected in place
    below.
  - The prerequisites 3.6.1, 3.6.2, 3.6.4 and 3.7.1 remain unticked, and no
    pull request covers them. PR #637 (localization guides and locale
    catalogues) is open and bears on how `check`'s advice text is localized.

- **`main` advanced 5 more commits before implementation began (2026-11-17).**
  The branch was rebased from `40a85aa5` onto `45a436bc`. The changes that
  affect this plan are:
  - `main` took ADR 0016 (`adr-0016-question-mark-is-a-branch.md`) and merged
    ADR 0015 (`adr-0015-brain-trust-lint-driver-interfaces.md`). The
    exit-contract ADR is therefore 0017; see the renumbering discovery above.
  - `installer/src/bootstrap/run.rs` and `driver_run.rs` were reworked again
    (`0d0f9450`). `prepare` still takes a `BootstrapLayout` and the `cargo`
    program name, still returns the same four-field `PreparedRun`, and
    `driver_run::execute` still takes `(&PreparedRun, &OsStr, &[OsString])`.
    Promoting the pair is therefore still an additive change.
  - `installer/src/bootstrap/child_path.rs` gained the Windows `PATH`
    adjustment. `RunOptions::inherited_path` already carries it, so `check`
    must read `PATH` once at the composition root exactly as forwarding does.
    A `None` there would silently skip the Windows driver probe fix.

- **`EP-M0` prototype results (2026-11-17).** Run against the real installation
  on this host: Dylint 6.3.0, suite built by `nightly-2026-05-28`, snapshot
  `e4608a25`, in a scratch crate under `/tmp/ep-m0`. The transcripts are
  abridged in each entry.
  - **H-1 (not falsified).** `cargo dylint --lib=whitaker_suite --no-metadata
    -- --all-targets` loaded exactly the recorded library and reported the
    suite's `module_must_have_inner_docs` diagnostic. The unsuffixed
    `--lib whitaker_suite` form searches `DYLINT_LIBRARY_PATH` *and* the
    `target/release` directories of the current workspace's metadata entries,
    so `--no-metadata` is required to make the recorded directory the only
    source. `--lib=NAME` (one token) is accepted, which is what keeps
    `DylintOption` renderable as single tokens.
  - **H-2 (not falsified; reports ambiguity).** With two `whitaker_suite`
    libraries in one `DYLINT_LIBRARY_PATH` directory, Dylint 6.3.0 exits `1`
    with ``Found multiple libraries matching `whitaker_suite` `` — it does not
    choose the toolchain that matches. `check` must therefore refuse a
    user-managed path holding more than one suite, with reason key
    `provision.user-path`, rather than letting that error reach the user.
  - **H-3 (not falsified; `AX-DYLINT-EXIT` confirmed).** Statuses observed:
    `1` for a deny-level lint (`DYLINT_RUSTFLAGS="-D warnings"`), `1` for a
    compile error, `1` for `-- --no-such-flag`, `2` for an unknown option
    before Dylint's `--`. Note that `DYLINT_RUSTFLAGS="-D
    module_must_have_inner_docs"` did *not* deny, while `-D warnings` did, so
    the driver overrides a per-lint command-line level; this does not affect
    the exit contract but matters to any future test that pins a level.
  - **H-4 (not falsified).** Promoting `prepare`/`execute` is a visibility
    change only; `run_inner` keeps composing them, so the bootstrap's own
    tests continue to exercise one path. Verified when the promotion landed.
  - **H-5 (not falsified).** Preflight plus run is forwarding's own code split
    in two, so the delta is zero by construction. Measured with `hyperfine`:
    the warm run is 76 ms ± 4 ms and its only subprocess, `cargo dylint
    --version`, is 19 ms ± 4 ms. Forwarding pays that probe too.
  - **H-6 (deferred).** The installation lock does not exist yet, so there is
    nothing to race. Its two-process test belongs to `EP-M2`.
  - **H-7 (not falsified; cache invalidation works).** On a warm Dylint target
    directory:
    - changing `DYLINT_RUSTFLAGS` between two runs re-ran `cargo check`
      (33.4 s) and reverting it re-ran again; an unchanged value was cached
      (0.01 s);
    - creating a `dylint.toml`, changing its contents, and changing its
      `[whitaker] experimental` list each re-ran the check (32–36 s), while
      an unchanged file was cached (0.01 s);
    - the same holds for `DYLINT_TOML`, which — discovered here — carries the
      TOML *text*, not a path. Passing a path parses the path as TOML and
      fails at the last column of the path.
    The selection and experimental carriers named in `CHK-INV-SELECT-MAP` are
    therefore both safe rebuild inputs.
  - **Unknown experimental names are fatal, as recorded.** A `[whitaker]
    experimental` entry naming a lint the suite does not have is reported by
    the driver as ``could not read configuration file`` and exits `101` from
    `rustc`, reaching `check` as a passed-through non-zero status.

- **The `host.data-dir` end-to-end case cannot be constructed by unsetting
  `HOME` and `XDG_DATA_HOME`.**
  - Evidence: `SystemBaseDirs::new()` wraps `directories-next`, which reads
    `XDG_DATA_HOME`, then `HOME`, and then falls back to the passwd entry. On
    this host, running the binary with `HOME` unset or empty found the real
    installation and reported `provision.refused` (the recorded snapshot is
    missing), not `host.data-dir`. `SystemBaseDirs::new()` therefore returns
    `Some` in every environment a test can construct without a fake passwd
    database.
  - Impact: the end-to-end host case uses the other deterministic host
    failure, `exec.spawn`: a user-managed library path holding a suite marker,
    with the child `PATH` pointed at an empty directory so `cargo` cannot be
    found. The `host.data-dir` key stays covered by the classification table,
    the exit-composition proof, and the router's unit tests. Recorded as part
    of D-9.

- **A lost `check --help` write prints no reason key unless the router
  reports it.**
  - Evidence: `whitaker check --help` into `/dev/full` exited `5` as the
    contract requires, but standard error was empty: the outcome came from
    `lost_diagnostic`, which only maps the code, while `messages::report` is
    called inside `run_check_command`, which the display path never reaches.
  - Impact: `print_clap_error` now calls `check::report_outcome` for a
    `check` route whose first diagnostic was lost, so the `exec.output` key is
    printed on the stream that still works. The end-to-end test asserts both
    the code and the key.

- **Once `check` can install, an "unprovisioned machine" test can provision
  the host that runs it.**
  - Evidence: the first `EP-M2` run of the root-CLI suites had
    `an_uninstalled_machine_is_refused_with_the_provisioning_code` (end-to-end)
    and `A user-managed library path without the suite is refused` (feature)
    still invoking `whitaker check` with no `--no-install`. Both would now
    reach the repair path: with an empty `XDG_DATA_HOME` the repair runs a real
    network install, and with the developer's own data root it silently
    answers from their installation.
  - Impact: the end-to-end case was replaced by
    `no_install_refuses_an_unprovisioned_machine`, the feature scenario runs
    `check --no-install`, and every scenario that must stay unprovisioned
    creates its own empty `XDG_DATA_HOME` and, where it asserts the absence of
    a child process, its own recording stub. Recorded under D-10.

- **A refusal that no longer has an installation fixture has nothing to
  observe "Dylint never started" against.**
  - Evidence: the user-managed-path feature scenario deliberately creates no
    installation, so `then_dylint_never_started`'s fixture lookup failed with
    "an installation fixture should be present" rather than passing.
  - Impact: that scenario now installs a recording stub `cargo` on `PATH` and
    asserts the stub's log was never written, which is the same promise
    observed without an installation.

- **`mapsplice` could not apply the `EP-M3` roadmap edit.**
  - Evidence: `mapsplice replace docs/roadmap.md 3.5.2 fragment.md` failed
    closed with `dependency anchor '3.5.2' was not found in the target
    roadmap`, even for a fragment identical to the anchor. `mapsplice insert
    --after docs/roadmap.md 3.5.3 fragment.md` did succeed, but the preview
    renumbered unrelated later sub-tasks (`2.2.11` to `2.2.10` among them) and
    normalized blank lines far outside the edit.
  - Impact: `docs/roadmap.md` was edited by hand. The two changes needed
    (narrow 3.5.2 and add 3.5.4 after 3.5.3) introduce no new sibling before
    an existing numbered item, so hand editing renumbers nothing and the
    preview's churn was rejected rather than committed.

- **A witness that an outcome class is inhabited is not a proof that every
  path reaches it, and the plan's earlier wording conflated the two.**
  - Evidence: the completeness assessment read "a reachability counterexample
    for each outcome class" as a claim over the flow. The Kani witnesses in
    `exit.rs` construct an outcome and then fail deliberately, so they prove
    the arm exists and maps to its status; before this round they covered four
    of the eight outcome shapes, and the flow witnesses covered five of the
    seven outcomes the flow can reach.
  - Impact: the gap was closed by evidence rather than by wording. Every exit
    class now has a constructor witness, including the reserved
    `ConfigurationFailed`; every outcome the flow can reach has a scripted
    witness, including `RepairFailed`, `RunFailed` and `OutputFailed`; and the
    one class no flow reaches in phase one is named as such in
    `flow/witnesses.rs` instead of being implied covered.

- **The flow model fixes its reason and fault after one symbolic sample.**
  - Evidence: `ProbePort::new` samples `reason` and `fault` once, so a single
    run carries one repairable reason and one host fault through every branch.
    That is enough for the flow invariants, which never compare two different
    reasons, but it does not prove that, say, a `Host(Io)` preflight and a
    `Host(Spawn)` repair failure propagate distinctly.
  - Impact: the limit is stated in the module and in D-11 rather than papered
    over. Distinct propagation is covered by `cli/check/tests/faults.rs`,
    which pins every arm of both fault tables, and by the classifier's
    ten-case table.

- **A snapshot fixture's installation record is only valid for the fixture
  that created it.**
  - Evidence: H-6's first two-process run installed into the destination
    fixture instead of finding it ready. The holder had written the *source*
    fixture's `bootstrap.json`, whose record names that fixture's snapshot
    commit; the destination's own record names a different one, so the
    destination legitimately read its installation as stale.
  - Impact: the test parks the destination's own record outside the
    installation and has the holder put it back under the lock. The rule for
    any future two-process test is that a fixture's record travels with the
    installation it describes, never across fixtures.

## Decision log

### D-1: split the roadmap item (accepted 2026-11-17)

- **Decision.**
  - Narrow roadmap item 3.5.2 to the four behaviours it names: routing, lazy
    repair, `--no-install`, forwarded Cargo arguments, plus the exit contract.
    Its `Requires` becomes 3.5.1 alone.
  - Add a new roadmap item, 3.5.4. It extends `whitaker check` with the
    configuration flags (`--config`, `install.lazy`), the rule-selection flags
    and the install-behaviour flags. It requires 3.5.2, 3.6.2, 3.6.4 and
    3.7.1.
  - The new item gets its own ExecPlan when its prerequisites land.
  - This plan is phase one. The phase-two obligations are recorded under
    `Verification plan` so that phase one's interfaces are designed for them.
- **Rationale.**
  - The behaviours named in the item do not depend on rule selection,
    presentation flags or the new install flags.
  - Waiting keeps users on the forwarding workaround through all of 3.6 and
    3.7.
  - The exit contract is the riskiest part and is best settled early.
  - A separate item keeps each roadmap entry's "done" honest. A note inside
    3.5.2 would leave the item half done for months.
- **Alternatives.**
  - Strict ordering: start nothing until 3.6.1, 3.6.2, 3.6.4 and 3.7.1 merge.
    This is coherent, changes nothing in the roadmap, and delays delivery.
  - Keep one roadmap item with two internal phases. This was rejected because
    the item could not be ticked for months.
- **Affected upstream.**
  - `docs/roadmap.md` 3.5.2 (its wording and `Requires` clause).
  - A new 3.5.4.
  - `docs/whitaker-cli-design.md`, where the status table names the items.
  - Edits are made with the `mapsplice` tool in `EP-M3`.
- Approving authority: the user, who accepted it on 2026-11-17.
- Date/Author: 2026-11-11, planning agent, revised after expert review.

### D-2: the `check` exit contract, ADR 0017 (accepted 2026-11-17)

- **Decision.** The new ADR is
  `docs/adrs/adr-0017-whitaker-check-exit-status-contract.md`. The codes are:
  - Dylint ran: its status is passed through unchanged.
    - `0` means success.
    - `1` means Dylint or Cargo reported failure: a lint denial, a compile
      error, a rejected Cargo argument, or an internal Dylint error.
    - `2` from Dylint means Whitaker assembled options that Dylint rejected,
      which is an internal fault.
    - `101` means `cargo dylint` panicked.
    - Signals map to `128 + n` on Unix. Windows codes pass through as
      `i32`.
  - `2`: a Whitaker usage error, as for every command.
  - `3`: a configuration error. Reserved now and used from 3.5.4.
  - `4`: a provisioning failure. The installation is incomplete, and repair
    was refused, failed, or did not converge. Under a user-managed path, the
    path does not contain the suite.
  - `5`: a host or execution failure. The data directory cannot be
    determined, an I/O error occurred, the installation lock failed,
    `cargo dylint` could not be started, or Whitaker could not write its own
    output.
  - `6` to `9`: reserved, binary-wide, for future operational outcomes.
  - Every operational failure also prints a stable, untranslated reason key
    on standard error, in the form `whitaker: error[<key>]`. The keys are
    `provision.refused`, `provision.repair-failed`,
    `provision.not-converged`, `provision.user-path`, `host.data-dir`,
    `host.io`, `host.lock`, `exec.spawn` and `exec.output`. `check --help`
    prints the exit table in its after-help text.
- **Rationale.**
  - `1` is taken by Dylint, and `2` is shared with clap usage errors in both
    Whitaker and `cargo dylint`. The small codes `3` to `9` are disjoint from
    every status `cargo dylint` documents (`0`, `1`, `2`, `101`), from
    signal-derived statuses (`129` to `192`), and from Windows NTSTATUS values
    seen as negative `i32`s.
  - One code per class of operator action keeps the contract coarse: fix the
    configuration (`3`), provision (`4`), or investigate the host (`5`). The
    reason keys let scripts tell causes apart without parsing prose.
  - Prior art: pytest uses small codes for operational classes. Ruff and
    ESLint use `0`, `1` and `2`, which Whitaker cannot copy because clap
    already owns `2`.
  - `sysexits.h` (`64` to `78`) was rejected. It is unfamiliar in Rust
    tooling and has no Windows convention.
- Approving authority: the user, who accepted it on 2026-11-17.
- Date/Author: 2026-11-11, planning agent, revised after expert review.

### D-3: readiness and repair use the bootstrap preflight

- **Decision.**
  - Readiness is the bootstrap's `prepare` query, made public as
    `bootstrap::preflight`. Dylint runs only through `bootstrap::run_prepared`
    on the `PreparedRun` that a successful preflight returned.
  - Repair is one coarse call to `run_install_for_whitaker`, with
    `InstallArgs::default()` plus `skip.no_update = true` and the verbosity
    flags. It never upgrades the managed clone. Partial repair arrives with
    3.7.x if at all.
  - The pure kernel classifies preflight failures:
    - Repairable: `NotInstalled`, `Metadata`, `LibrariesMissing`,
      `Snapshot`, `CargoDylint` and `DylintVersion`.
    - Host: `NoDataDirectory` and `Io`.
    - Spawn: only `run_prepared` can report it.
- **Rationale.**
  - There is one source of truth: what preflight approves is what runs.
  - The warm path costs what forwarding costs today. The only subprocess is
    `cargo dylint --version`, which `prepare` already runs.
  - It honours the ADR 0012 ownership split.
  - It removes the first draft's illegal states, a per-component missing set
    that the installer could not honour, and the unobservable "toolchain"
    component.
  - `dylint-link` is not checked. Running prebuilt libraries does not need
    it, and repair installs it anyway.
- **Alternative rejected.** "Detect readiness by running Dylint" was rejected
  because Dylint's exit `1` is ambiguous. Preflight sits before the spawn, so
  it gets the same benefit without the ambiguity.
- Date/Author: 2026-11-11, planning agent, from expert review.

### D-4: repair runs under an installation lock

- **Decision.**
  - The installer gains `orchestration::with_install_lock`, an exclusive `fs2`
    lock on `<whitaker_data>/install.lock`.
  - Inside the lock, `check`'s repair adapter runs preflight again. It
    installs only if preflight still fails with a repairable error.
  - A failure to take the lock is a host failure, exit `5`, with the reason
    key `host.lock`.
- **Rationale.**
  - Concurrent repairs would otherwise duplicate downloads and race on
    staging.
  - The second waiter finds the installation ready and does nothing.
- Date/Author: 2026-11-11, planning agent, from expert review.

### D-5: the other commands keep the 3.5.1 exit policy

- **Decision.**
  - `install`, `ls`, display requests and legacy forwarding keep the 3.5.1
    exit policy.
  - ADR 0017 reserves the meanings of `3` to `9` binary-wide, so that no
    command may ever use those codes differently.
  - ADR 0017 records, as an outstanding decision, that `install` aligns with
    these meanings no later than the 3.9 compatibility release.
- **Rationale.** Constraint 2. Changing those commands would alter a released
  contract outside this item.
- Date/Author: 2026-11-11, planning agent.

### D-6: the forwarded-argument boundary and the Dylint options set

- **Decision.**
  - Arguments after `--` go to `cargo check` through Dylint. This resolves
    the design's "to `cargo dylint` or the underlying cargo invocation"
    wording in favour of the latter.
  - `check` assembles `cargo dylint <W> -- <U>`, where `U` is exactly the
    user's forwarded arguments and `W` is drawn from a closed enum of
    Whitaker-owned Dylint options. That enum is fixed in `EP-M0`. The working
    hypothesis is `--lib whitaker_suite`, never `--all`, plus possibly
    `--no-metadata`.
  - Dylint-level options are not exposed in this item: `--fix`,
    `--keep-going`, `--manifest-path` and Dylint's own `-p`.
  - `whitaker check -p x` is a usage error (exit `2`), with clap's tip to use
    `-- -p x`.
  - The root `-q`/`-v` flags keep controlling Whitaker's own diagnostics only.
    To quieten Cargo, users write `-- -q`.
- **Rationale.**
  - `--all` also loads libraries from the user's workspace metadata.
  - A path with no suite library would exit `0` with no lints run.
  - The design surface is normative, and new flags are a design change.
- Date/Author: 2026-11-11, planning agent, from expert review.

### D-7: expert review disposition

Three reviewers examined the first draft and independently converged on the
following. Every material finding was adopted:

- Use the typed bootstrap API (D-3).
- Readiness is the bootstrap preflight (D-3).
- Repair is coarse (D-3).
- Repair runs under an installation lock (D-4).
- Host faults exit `5`, and every operational failure carries a reason key
  (D-2).
- The forwarded-argument boundary and the `--manifest-path` caveat are
  documented (D-6).
- The cache-invalidation hypothesis H-7 is added to `EP-M0`.
- The Kani and Verus artefacts and the CI paths are wired explicitly.
- A provisioned-host test fixture is specified.
- Repair scenarios run in-process with fake ports, because a binary-level
  repair cannot run offline until 3.7.1.
- `CheckFlow` is generic over a narrow port trait, so Kani never reaches
  `InstallArgs`.
- Red tests are run in focused loops and never committed in a red state.
- Tracing and metrics are added.
- The budgets are split.
- Phase two moves to its own roadmap item (D-1).

The review's strongest alternative was preflight, then repair, then retry. It
is now the design.

Two points were reviewed and kept:

- The Verus argument lemma is cheap, and its inner-`--` witness is useful.
  The proptest and an end-to-end argument-recording test carry the executable
  evidence. The plan says so plainly.
- Kani is preferred to Verus for the finite repair flow.

Date/Author: 2026-11-11, planning agent.

### D-8: `W`, the Whitaker-owned Dylint options, fixed by `EP-M0`

- **Decision.** `W` is exactly three tokens, in this order:

  ```plaintext
  --lib=whitaker_suite  --no-metadata
  ```

  so `check` assembles `cargo dylint --lib=whitaker_suite --no-metadata -- <U>`.
  `DylintOption` is a closed enum whose rendering is a single token per
  variant; the shell form `--lib whitaker_suite` is not used.
- **Rationale.**
  - `--no-metadata` is required, not optional: without it `--lib` also searches
    the workspace's `target/release` directories, so a project that happens to
    have built a `whitaker_suite` would run that library instead of the
    recorded one (H-1).
  - `--lib=NAME` keeps every Whitaker-owned token free of `--`, which is the
    precondition `CHK-LEM-ARGV` proves and `CHK-INV-ARGV` tests.
  - `--all` is excluded: it loads every discovered library, so a path with no
    suite would exit `0` having linted nothing (D-6).
  - `--fail-on-no-libraries` was considered and rejected as redundant: with a
    library selection given, a missing library already exits `1` (H-2's empty
    path case).
- **Alternatives.**
  - `--lib whitaker_suite` as two tokens: rejected because it breaks the
    single-token invariant that the argument lemma rests on.
  - `--no-deps`: rejected. `check` should check the workspace as `cargo check`
    does, including path dependencies.
- Date/Author: 2026-11-17, implementation agent, from `EP-M0` evidence.

### D-9: `EP-M1` implementation deviations (accepted 2026-11-17)

- **Decision.** The milestone keeps the plan's contracts and differs from its
  sketch in these ways. Each is a placement or shape choice, not a change to an
  observable promise.
  - **The port is `CheckServices`, and the library mode stays in the
    adapter.** `preflight() -> Result<CheckPrepared, CheckPreflightError>` and
    `check_run(CheckPrepared, &[OsString], &mut dyn Write) ->
    Result<i32, CheckRunError>`, both in `src/ports.rs`. `LibraryMode` lives
    in `src/adapters/check/mod.rs`, so only the adapter reads
    `DYLINT_LIBRARY_PATH` and the command layer is handed a value. This is the
    environment-injection rule applied to the process environment rather than
    to a `mockable::Env` trait, because the adapter is the composition edge
    for the child process.
  - **`CheckPrepared` is an enum, not the raw `PreparedRun`.** The bootstrap
    route carries `PreparedRun`; the user-managed route carries the
    `DYLINT_LIBRARY_PATH` value. The flow never inspects it.
  - **Port errors are classified enums.** `CheckPreflightError` adds
    `UserPathHasNoSuite` to the bootstrap's typed errors, and `CheckRunError`
    separates `Bootstrap`, `Spawn` and `Output`. `RunFault` therefore carries
    `Provisioning`, `Spawn`, `Host` and `Output` separately, so the exit
    mapping never re-derives a cause from an error string.
  - **`decide` and `run_check_flow` take the preflight by value.**
    `Step<P>` can only hold a `P` that a `Ready` answer produced, which makes
    "Dylint runs only against a freshly approved installation" structural.
  - **`src/cli/check/flow/mod.rs` was split at the 400-line budget.** The tests
    live in `flow/tests.rs` and the Kani harnesses in
    `flow/verification.rs`.
  - **Telemetry is defined in the adapter and emitted at the command
    boundary.** `src/adapters/check/telemetry.rs` owns the span and counter
    names; `run_check_command` opens the span and records a refusal.
    `whitaker.check.repairs` is deferred to `EP-M2`, which is the first
    milestone that can repair.
  - **The installation lock lands here, unused.**
    `installer/src/orchestration/lock.rs` adds `with_install_lock`, which
    `EP-M2` calls. Landing it with the installer promotion keeps the
    bootstrap-layout change in one review.
  - **The `host.data-dir` end-to-end case is replaced by `exec.spawn`.** See
    the `Surprises & discoveries` entry: the plan's environment manipulation
    cannot reach `NoDataDirectory` on this platform.
  - **A lost `check --help` write reports its key from the router.** The exit
    code alone was not enough to satisfy ADR 0017's "every operational failure
    prints a reason key".
- **Rationale.** Each deviation either enforces an existing rule (environment
  injection, file budget, structural freshness) or makes an existing promise
  observable (the reason key). None adds a command, a flag, a code, or a key.
- **Alternatives.**
  - Keep `LibraryMode` on the port. Rejected: it would make the command layer
    read the environment.
  - Keep `RunFault::Spawn` merged into `Host`. Rejected: the exit code is the
    same, but a spawn failure and a data-directory failure point at different
    operator actions.
  - Defer the lock and the whole installer promotion to `EP-M2`. Rejected: the
    promotion is what `EP-M1` runs on, and splitting it would leave two
    reviews of the same bootstrap surface.
- Approving authority: this plan, under the approval the user gave on
  2026-11-17; no contract changed.
- Date/Author: 2026-11-17, implementation agent.

### D-10: `EP-M2` implementation deviations (accepted 2026-11-18)

- **Decision.** The milestone keeps the plan's observable promises and differs
  from its sketch in these ways.
  - **The policy is a pure function in `src/cli/check/policy.rs`.** The command
    asks the port `library_path_is_user_managed() -> bool` and derives
    `policy_for(no_install, user_managed_path)`. The CHK-INV-POLICY table and
    its four cases live beside it, so the truth table is checked without a
    process or an environment.
  - **`CheckServices` gained `repair(&mut dyn Write) ->
    Result<(), CheckRepairError>`.** The error is classified at the port
    (`Install`, `Lock`, `DataDirectory`, `Host`) and mapped by the command:
    install failures and non-convergence stay `provision.*` on exit `4`, while
    lock, data-directory and other host failures become `host.*` on exit `5`.
    The plan sketched a repair call inside the adapter; splitting it at the
    port is what lets the in-process feature prove the decision without
    installing anything.
  - **Repair re-preflights inside the lock and returns early when already
    ready.** A second process that waited on the lock therefore does not
    reinstall, and a repair that fails to converge reports
    `provision.not-converged` from the fresh preflight rather than from a
    mutable flag.
  - **The installer's standard output is redirected to standard error by the
    adapter.** `repair` hands `run_install_for_whitaker` a fresh
    `std::io::stderr()` for its `stdout` parameter, and the `CheckServices`
    trait never passes the process's stdout handle. CHK-INV-STDOUT is
    therefore structural, not a convention.
  - **The repair announcement is Whitaker's own, on standard error**, and adds
    a CI-specific line advising a cached `whitaker install` followed by
    `check --no-install`.
  - **Telemetry is emitted at the command boundary.** The command wraps its
    ports in `RecordedPorts`, which calls `telemetry::record_repair` after each
    repair and `telemetry::record_preflight` after each readiness question, so
    the flow stays free of adapters and both questions the repair route asks
    are recorded. Tests assert a non-zero counter through `run_check_command`
    under a `DebuggingRecorder`, and one event per readiness answer through a
    subscriber.
  - **A user-managed path overrides libraries but not the bootstrap.** When a
    recorded installation exists, preflight runs with the user's directory as
    `RunOptions.library_path`, so the snapshot and driver still come from the
    bootstrap; with no usable record, `suite_present` decides and a missing
    or ambiguous suite is `provision.user-path`. This refines the plan's
    "user path means direct" sketch.
  - **The two-process lock test runs the test binary as the child.** An
    ignored probe test is selected with `--exact`, names the lock file through
    an environment variable, and reports `free` or `contended` on standard
    error; the parent asserts `contended` while the lock is held and `free`
    after the closure returns.
  - **The tests that must stay unprovisioned were made explicitly so.** See the
    two `Surprises & discoveries` entries above: `--no-install`, an empty
    `XDG_DATA_HOME` per scenario, and a recording stub where the absence of a
    child process is the assertion.
- **Rationale.** Each deviation either makes an existing promise structural
  (stdout purity, repair-once, readiness inside the lock) or makes the decision
  testable without a network, a process or the developer's real installation.
  None adds a command, a flag, a code, a key or a persisted format.
- **Alternatives.**
  - Repair inside the adapter behind a boolean. Rejected: the in-process
    feature could not then prove the policy table, and the command would lose
    the classification.
  - Pass the process stdout to the installer and trust it not to write.
    Rejected: purity would depend on the installer's behaviour rather than on
    the type it is given.
  - A `no_install` flag on `repair`. Rejected: the policy already decides
    whether repair is reached, and a second gate would be dead in every test
    that exercises it.
- Approving authority: this plan, under the approval the user gave on
  2026-11-17; no observable contract changed.
- Date/Author: 2026-11-18, implementation agent.

### D-11: final review round (accepted 2026-11-20)

- **Decision.** Both ExecPlan assessments were re-triaged against the tree at
  head `d227cfa6`. Every correctness finding is fixed; every finding that is a
  wording, scope or evidence gap is closed by changing the artefact the finding
  named. Nothing is deferred without a name and a reason.
  - **Readiness and the run re-check now agree.** Readiness approves an
    installation that holds a Whitaker suite for its toolchain, but the run's
    re-check applied Dylint's exactly-one rule, so a two-toolchain
    installation was approved by one step and rejected by the next. The
    re-check now asks the same question as readiness through
    `prepared_suite_available`, and
    `a_two_toolchain_installation_stays_available_to_the_run` would fail under
    the old rule. *Superseded by D-12:* the premise was wrong. Dylint 6.3.0
    exits `1` when `--lib` names two libraries, so a two-toolchain directory
    is not runnable at all; both steps now apply the exactly-one rule.
  - **A repair whose notice cannot be written is an output failure.**
    `CheckRepairError::Output` carries the stream error, the flow maps it to
    `CheckOutcome::OutputFailed`, and the outcome prints `exec.output` with
    exit `5`. The fault table and the repair-flow test both pin the arm.
  - **`CHK-INV-ARGV` is one end-to-end property.**
    `tests/property_check_argv.rs`
    parses a generated command line through the real clap surface, checks the
    derived settings and `--no-install`, and requires the assembled vector to
    be Whitaker's prefix followed by the user's tokens byte for byte. 4,096
    cases run per test, and three coverage counters require at least a tenth of
    them to carry an empty user vector, an inner `--`, and a first token
    starting with a dash.
  - **`CHK-LEM-ARGV` is the plan's general lemma.**
    `verus/whitaker_check_argv.rs`
    defines `assemble(W, U) = W ++ ["--"] ++ U`, a recursive
    `first_separator`, `cargo_args` and `has_no_separator`, and proves by
    induction on `|W|` that the first separator sits at `|W|` and that
    everything after it is `U`, for any separator-free `W` and any `U`
    including one that holds `--`. The concrete two-option `W` is a corollary.
    The module states what it does not model — how a token renders to bytes —
    and names the property tests that cover that.
  - **Reachability is witnessed per class, and the wording matches.** Every
    exit class now has a constructor witness, and every outcome the flow can
    reach has a scripted witness: `RepairFailed`, `RunFailed` and
    `OutputFailed` were added. `ConfigurationFailed` stays witnessless because
    no flow reaches it in phase one, and `flow/witnesses.rs` says so.
  - **The exit contract no longer implies total disjointness.** `exit.rs`
    states that `Linted(code)` passes any status through, that Dylint may
    itself exit `3`, `4` or `5`, and that the reason key is what separates the
    two. The codes remain disjoint from the statuses `cargo dylint` documents,
    which is the claim ADR 0017 makes.
  - **Telemetry reaches a subscriber.** The preflight class, the chosen
    decision and the repair outcome are emitted as static-labelled events, and
    a test captures all three through `tracing`'s collection path rather than
    inspecting the call sites.
- **Pre-merge documentation rows.** CodeRabbit's User-Facing Documentation
  check named two gaps: no 0.4.0 migration guide, and a README whose quick
  start still led with `whitaker-installer`. Its Developer Documentation check
  named a third: the bootstrap boundary published `PreparedRun`, `preflight` and
  `run_prepared` without describing them. All three are now written rather
  than argued: `docs/migrations/0.4.0.md` covers the new default path, the
  source-breaking `InstallLock` variant, the additive bootstrap API and the
  exit-status contract; the README presents `whitaker check` first and keeps
  the standalone installer as the explicit route; and the developers' guide
  lists every public bootstrap entry point and what `preflight` verifies.
- **CodeScene delta, second round.** The first delta was clean, but the review
  round's new code introduced eight similar exit witnesses and two similar
  `ProbePort` constructors. Both are refactored: `exit_witness!` takes the
  harness name and the two message strings so each witness states only its
  outcome, and `ProbePort::from_plan` holds the shared construction. The
  message strings are unchanged because `scripts/run-kani.sh` matches them.
  `cs delta` then reported no issues in the checked changes.
- **Scope escalation.** The tolerances allow 30 code-and-test files and 2,500
  net lines, and 12 documentation files and 800 net lines. Measured against the
  merge base `45a436bc` on the final tree, this branch touches 73 code-and-test
  files for 9,001 net lines and 10 documentation files for 646 net lines. Code
  and tests exceed their budget by 43 files and 6,501 lines; documentation is
  inside its budget. The figure reported to the user at the time — 46 files and
  5,165 net lines — counted production files only and omitted the test modules,
  so the correction is larger, not smaller; it does not change the decision
  that report supported. The budget was exceeded by review-driven work across
  three review rounds — defects, proofs, end-to-end tests, module splits forced
  by the 400-line rule and documentation — not by a widening of the feature.
  The escalation was reported to the user, who authorized continuing to merge;
  no observable contract, flag, code, key or persisted format was added beyond
  the plan.
- **Negative controls.** Two mutations were run on the final tree and then
  reverted. Emitting the `Lib` option as `--` followed by `whitaker_suite` made
  the argument property and four named tests fail. Classifying
  `BootstrapError::Io` as repairable made the classifier's `io` case, the fault
  table's `case_2_host` and the flow's `case_2_io` fail. That control's
  obligation belongs to the `rstest` table, not to the Kani host harness, which
  constructs `Preflight::Host` directly and never consults the classifier.
  Transcripts and results are recorded in D-12.
- **Dispositions that change no code.** `CI` is read once, in
  `src/adapters/check/mod.rs`, which is the adapter's environment composition
  root and already reads the library mode; the command does not read it. The
  repair branches are driven through the `RepairSteps` seam, so the only
  delegation left untested is `SystemSteps`' one-line call into the installer,
  which the end-to-end tests exercise against a real installation. The flow
  model's single symbolic reason and fault are stated in `verification.rs` and
  under `Surprises & discoveries`.
- **Rationale.** The completeness assessment's two defects were real: each
  described a path where the code said one thing and did another. The proof
  findings were also real, and the answer is a stronger theorem and more
  witnesses, not a narrower statement.
- **Alternatives.**
  - Keep the fixed-prefix Verus model and describe it as the plan's lemma.
    Rejected: it would not survive an option being added, which is the case
    the lemma exists to rule out.
  - Record the reachability gap as accepted risk. Rejected: three witness
    macros and three scripted scenarios close it for less work than a
    rebuttal.
  - Revert the over-budget changes to satisfy the tolerance. Rejected: they
    are the review's findings, and reverting a defect fix to fit a budget
    would be the wrong trade.
- Approving authority: the user's instruction to take PR #603 through
  babysitting to merge, after the escalation above was reported.
- Date/Author: 2026-11-20, implementation agent.

### D-12: completeness assessment round two (accepted 2026-11-20)

- **Decision.** The second completeness assessment
  (`EP352-COMPLETE-20261120T060853Z`) found one correctness defect and four
  evidence obligations. Every finding is actioned against the tree. The D-11
  statement it falsified is marked superseded in place rather than deleted, so
  the record keeps what was believed and why it was wrong.
- **A recorded directory with two suites could not lint.** Dylint 6.3.0 exits
  `1`, the status it also uses for a lint denial, when `--lib=whitaker_suite`
  resolves to two libraries. D-11's "agreement" therefore made both steps
  approve a run that could only be reported as a lint result.
  `recorded_suite_present` now applies the user path's exactly-one rule;
  readiness (`preflight_recorded_with`) and the run re-check
  (`run_bootstrapped`) both call it, and `suite_available` and
  `prepared_suite_available` are deleted. The new
  `CheckPreflightError::RecordedPathAmbiguous` classifies as the repairable
  `RepairableReason::LibrariesAmbiguous`, labelled `libraries_ambiguous` and
  reported as `provision.refused`, or as `provision.not-converged` when a
  repair leaves the directory in place. Evidence:
  `a_recorded_directory_with_one_suite_is_approved`,
  `a_recorded_directory_without_the_suite_is_missing`,
  `a_recorded_directory_with_two_suites_is_ambiguous`, the classifier and
  fault-table cases, and the end-to-end
  `a_recorded_directory_with_two_suites_is_refused_before_dylint`, which pins
  exit `4`, `provision.refused`, the diagnosis, and that the stub child never
  started.
- **The user-managed route no longer skips an undeterminable data directory.**
  `preflight_user_managed` now delegates to
  `preflight_user_managed_in(dirs, path)`, and a `None` directory returns
  `BootstrapError::NoDataDirectory`: the `host.data-dir` fault at exit `5`, the
  answer the recorded route already gave. Running the user's path instead would
  silently ignore a record that might contradict it.
  `a_user_managed_path_without_a_data_directory_is_a_host_fault` drives the
  missing-directory case with a value, and
  `a_user_managed_path_with_no_record_runs_directly` keeps the legacy fallback.
- **The production repair wiring is exercised through seams.**
  `repair_request(settings, ci)` is the CI decision and takes the composition
  root's `std::env::var_os("CI")` answer as a value;
  `the_ci_notice_follows_the_environment_value` pins both branches without
  mutating the process environment. `repair_under_lock` now takes its
  `RepairStreams` from the caller instead of choosing standard error itself, so
  the adapter's composition root builds the only production pair while
  `tests/repair_flow.rs`'s marker-routing test proves installer markers reach
  the diagnostics stream. `tests/e2e_check_streams.rs` holds the same promise
  against the real binary.
- **H-6 has its two-process test.** `tests/e2e_check_lock.rs` re-invokes the
  test binary in an ignored mode so a holder can take `install.lock`, write a
  ready marker, hold for 3 s, and only then install the record. A `check`
  started on the marker must wait at least 1.5 s, exit with the stub's status
  `23`, and never print the repair notice. The first run was red for the right
  reason: the source fixture's record named a different `SnapshotFixture`
  commit than the destination held, so the waiting check spent a repair
  installing. Parking and restoring the destination's own record is the fix,
  and the lesson is that a snapshot fixture's commit is not interchangeable
  between installations.
- **Negative-control transcripts.** Five mutations were applied to the final
  tree, run, and reverted; each touched file was confirmed unchanged afterwards.
  - *Argument assembly, unit level.* Emitting `Lib` as `--` followed by
    `whitaker_suite` failed
    `an_empty_user_suffix_still_ends_with_the_separator` and
    `the_assembled_vector_puts_the_separator_after_the_owned_options`
    (`2 passed; 2 failed`; `/tmp/control-argv-unit.out`).
  - *Argument assembly, property level.* The same mutation failed
    `assembly_covers_every_named_input_shape` cases `1`–`3` and
    `every_forwarded_argument_reaches_the_assembled_vector_unchanged`
    (`5 passed; 4 failed`; `/tmp/control-argv-property.out`).
  - *Io classification.* Classifying `BootstrapError::Io` as repairable
    failed `every_bootstrap_error_has_a_class::case_09_io`,
    `faults::a_run_failure_is_classified_by_what_can_fix_it::case_2_host` and
    `policy::a_host_fault_is_reported_without_advice::case_2_io`
    (`119 passed; 3 failed`; `/tmp/control-io-classify.out`).
  - *Verus separator lemma.* Duplicating the separator in `assemble` made
    `verus/whitaker_check_argv.rs` report `4 verified, 3 errors`, each an
    assertion failure in the separator lemmas
    (`/tmp/control-verus.out`).
  - *Kani at-most-once repair.* Calling `ports.repair()` twice in
    `repair_then_lint` failed `verify_check_flow_repairs_at_most_once` with
    `Failed Checks: "repair must run at most once"`
    (`src/cli/check/flow/verification.rs:231`), while the four harnesses
    before it stayed successful (`/tmp/control-kani.out`). Two weaker attempts
    were inert and are not counted as controls: letting the second decision
    repair cannot fire, because `repair_then_lint` asks with
    `repair_allowed: false`, so only a second unconditional call reaches the
    guard. *Amended by D-14:* the arm alone is inert for that reason, but the
    two-part mutation this plan specifies — the caller's policy **and** a
    repairing `Step::Repair` arm — does fire, and was run.
- **Observability warning made explicit.** `repair_fault` and `run_fault`
  still classify to bounded enums, but the cause is no longer discarded:
  `CheckRunner::repair` and `CheckRunner::run` log it with
  `tracing::warn!(error = %error, …)` before returning the verdict, so the
  message stays in the log and never reaches a label. `CHECK_REPAIRS` counts
  the install call, and its documentation and metric description now say so:
  convergence is the flow's `not-converged` decision label, not the counter's
  `outcome`.
- **Rationale.** The correctness finding was right for a reason the earlier
  round had missed: the readiness helper was tested against another helper
  rather than against Dylint's selection rule. The evidence findings were also
  right; each is answered with the artefact the plan named, not with a
  reference to an earlier summary.
- **Alternatives.**
  - Keep `suite_available` and pass Dylint a directory it cannot resolve.
    Rejected: Dylint's status would arrive as a lint result.
  - Treat the undeterminable data directory on the user route as an
    intentional fallback. Rejected: the record cannot be trusted when the
    directory is unknown, and the recorded route already reports the fault.
  - Keep the in-process lock test as H-6 evidence. Rejected: lock contention
    and re-checking readiness are different claims, and only a second process
    can produce the first.
- Approving authority: the user's instruction to take PR #603 through
  babysitting to merge.
- Date/Author: 2026-11-20, implementation agent.

### D-13: proofs assessment (accepted 2026-11-20)

- **Decision.** The proofs assessment (`EP352-PROOFS-20261120T060853Z`, head
  `aeefeada`, assessed plan blob `d61ce58a` at revision 4) found the proofs
  substantive and well-founded within their stated models, and named four
  remaining concerns. Each is dispositioned here against the tree at head
  `26524e55`. Three controls this plan requires, and D-12 had not yet recorded,
  were run and are recorded now.
- **The three missing controls are recorded.** Each mutation touched the
  final tree, was run, and was reverted; the four touched files were then
  checked byte-identical against their recorded SHA-256 sums.
  - *Routing, Kani.* Routing `TokenTag::Check` to `Class::Forward` failed
    `verify_classify_is_total_and_disjoint` with
    `Failed Checks: "a first significant check token must select the check
    subcommand"` at `src/cli/dispatch.rs:216` (`1 of 362 failed`;
    `/tmp/control-routing-kani-3-5-2-whitaker-check-as-the-default-linting-path.out`).
  - *Routing, Verus.* Dropping the `Check` exclusion from `is_forward` made
    `./scripts/run-verus.sh cli` report `6 verified, 2 errors`, each a failed
    `assert(!is_forward(tokens))` in `lemma_predicates_partition_the_inputs`
    and in the bridge lemma
    (`/tmp/control-routing-verus-3-5-2-whitaker-check-as-the-default-linting-path.out`).
  - *Exit mapping, Kani.* Mapping `NotConverged` to `1` failed
    `verify_check_exit_composition` with
    `Failed Checks: "a provisioning failure must exit 4"` at
    `src/cli/check/exit.rs:239` (`1 of 46 failed`;
    `/tmp/control-exit-map-kani-3-5-2-whitaker-check-as-the-default-linting-path.out`).
  - The second-repair, argument-unit, argument-property, classifier and Verus
    separator controls remain those D-12 records; none was re-run.
- **The flow claim is bounded.** The plan no longer calls the flow
  exploration a "complete check of the real code". It is exhaustive over the
  scripted port's answer plans, which reuse one repairable reason and one host
  fault per run; mixtures of different reasons or faults are outside it, and
  `verify_check_flow_host_faults_never_repair` narrows to a first-preflight
  host fault. The observable consequences of a host result on the second check
  — no run and no second repair — remain covered by `F4` and `F3` over every
  path. Resampling per call would widen the claim, but no invariant the flow
  states needs it.
- **The argument claim is bounded.** The plan now states that the Verus model
  gives every forwarded token the same payload-free value, so it establishes
  the separator's position and the recovered sequence's length and classes, not
  its bytes or pairwise order. Byte-level preservation is `CHK-INV-ARGV`'s
  4,096-case property, which is executable evidence, not an unbounded proof.
  The proof module's header says the same.
- **Exit and routing claims are precise.** The plan now records that the Kani
  exit harness checks key *presence* and the unit table pins each key's value;
  that the Verus routing proof is about a model bridged to a model of
  `classify`, while Kani checks the production function over sequences of at
  most six tokens; and that `ConfigurationFailed`'s witness establishes the
  mapping to `3`, not command-level reachability.
- **The classifier-mutation point was already accepted.** D-11 states that
  the classifier control's obligation belongs to the `rstest` table, not to the
  Kani host harness, which constructs `Preflight::Host` directly. That sentence
  needed no change; the new transcripts close the rest of the requirement.
- **Rationale.** The assessment read the flow claim as broader than the
  harness supports, and the plan's "complete check" sentence was the source.
  Correcting the sentence is more honest than widening the harness to a claim
  no invariant needs. The missing controls were required by this plan's own
  non-vacuity sections, so recording them closes planned work rather than
  answering a suggestion.
- **Alternatives.**
  - Resample the reason and fault per call in `ProbePort` so the broader claim
    becomes true. Rejected for this change: it widens a proof harness no flow
    invariant depends on, and per-fault propagation is already pinned by the
    fault tables.
  - Model user-token payloads in Verus. Rejected: `CHK-INV-ARGV` already
    exercises the production rendering with 4,096 generated cases, and a
    payload model would need a byte-level bridge to the Rust code that the
    lemma does not otherwise need.
- Approving authority: the user's instruction to take PR #603 through
  babysitting to merge.
- Date/Author: 2026-11-20, implementation agent.

### D-14: assessments round three, and the controls they named (accepted 2026-11-20)

- **Decision.** The completeness and proofs assessments
  (`EP352-COMPLETE-20261120T060853Z` and `EP352-PROOFS-20261120T060853Z`, both
  given at head `175c7e04` against plan revision 6) name five items. Two are
  code defects, three are evidence obligations. Each is actioned against the
  tree; the two non-vacuity lines that named the wrong obligation are corrected
  in place, and the transcripts they ask for are now inline.
- **The run re-check keeps the route's own reason.** A prepared value now
  carries the route that approved it: `CheckPrepared::Bootstrap` holds a
  `LibraryRoute` of `Recorded` or `UserManaged`, and `suite_rule(route, path)`
  dispatches to `recorded_suite_present` or `suite_present`. Readiness and the
  run's re-check both call it, so a user-managed path that loses its suite
  after the preflight keeps the user-path reason instead of being described as
  a defect of the installed libraries. Evidence:
  `adapters::check::tests::recheck`'s six-case table,
  `a_user_path_that_changed_after_readiness_keeps_the_user_path_reason`,
  `a_recorded_directory_that_changed_after_readiness_keeps_its_own_reason`, the
  two new run-fault rows in `cli/check/tests/faults.rs`, and the `F7` harness.
- **A run-time user-path fault is refused, not reported as non-convergent.**
  `classified` maps `RunFault::Provisioning(UserPath)` to
  `CheckOutcome::Refused(UserPath)`: exit `4`, key `provision.user-path`, and
  the advice the route's readiness refusal already gives.
  `provision.not-converged` is reserved for a repair that ran and did not
  converge, and this path never spends one.
  `a_run_fault_the_flow_may_not_repair_reports_its_refusal_key` (`user_path`
  case) asserts the outcome, the zero repair count, the single run, the key,
  and the absence of `provision.not-converged`.
- **The production marker stream is tested at the production function.**
  `repair_production` selects a process-wide descriptor, which no in-process
  writer can substitute, so `adapters::check::tests::repair_streams` observes
  it from a child process: the child calls the production function with a
  scripted installer that writes a marker, echoes its own captured diagnostics
  between frame lines, and the parent asserts the marker never reached standard
  output, did reach standard error, and never entered the diagnostics frame.
  This closes the gap the earlier `repair_flow.rs` test left, because that test
  supplied its own writers.
- **The plan's two-part `F3` control was run and fires.** D-12 had run a
  stronger unconditional double-call control and inferred that a
  second-decision repair could not fire. The inference holds for the arm alone;
  with the caller's policy **and** a repairing `Step::Repair` arm, the arm is
  reachable and `F3` fails. D-12's sentence is amended in place rather than
  rewritten.
- **The Io obligation is re-pointed at the classifier tables.** `F5`
  constructs `Preflight::Host` directly and never calls the classifier, so no
  classifier mutation can fail it. The obligation is
  `every_bootstrap_error_has_a_class::case_09_io` and
  `policy::a_host_fault_is_reported_without_advice::case_2_io`, both re-run
  against the current tree.
- **The `check` port moved to its own file.** Carrying the route in
  `CheckPrepared::Bootstrap` took the ports module from 400 lines to 420, past
  the repository's 400-line limit. The check surface — `LibraryRoute`,
  `CheckPrepared`, the three error enums and `CheckServices` — now lives in
  `src/ports/check.rs` and is re-exported from `whitaker::ports` by name, so no
  call site changed. The parent is `src/ports/mod.rs` rather than
  `src/ports.rs` because the workspace denies
  `clippy::self_named_module_files`, which requires a directory module once
  `src/ports/` exists; it keeps the installer, inventory and forwarding ports
  plus `Services`. Both files are under 250 lines. Step 9 of this plan named
  this split as the remedy if the module outgrew the limit.
- **The transcript obligation is stated as an excerpt and met inline.** The
  reviewed version recorded results and `/tmp` paths, which a reviewer cannot
  read. `Outcomes & retrospective` now carries each control's mutation,
  command, and decisive output lines, and the requirement below says that is
  what a transcript means for this plan. Full logs stay under `/tmp` for the
  landing session.
- **Negative controls run this window.** Five mutations, each applied to this
  tree, run, and reverted; `grep -rn "NEGATIVE CONTROL" src/` then found
  nothing and `git diff --stat` matched the pre-control list. Every decisive
  line is in the transcripts subsection below.
  - Route re-check: `run_bootstrapped` applied the recorded rule to a
    user-managed path.
  - Marker stream: `repair_production` wrote installer markers to standard
    output.
  - User-path outcome: `classified` reported a user-path run fault as
    `NotConverged`, checked by both the command test and the `F7` harness.
  - `F3` two-part second-decision mutation, plus an instrumented run that
    names the second `repair` call.
  - Io classification, re-confirming D-12 against this tree.
- **Rationale.** Both code findings were real: the route that approved a path
  is a fact the prepared value has to carry, because re-deriving it from the
  environment after the fact could disagree with what readiness approved; and a
  refusal must not blame a repair the flow never ran. The evidence findings
  were also real: a test that supplies its own writers proves the helper, not
  the production choice, and a non-vacuity clause that names the wrong harness
  is worse than no clause, because it invites a control that cannot fail.
- **Alternatives.**
  - Re-derive the route at run time from `DYLINT_LIBRARY_PATH`. Rejected: the
    environment can change between the two steps, and the run must judge the
    path readiness approved.
  - Report `NotConverged(UserPath)` and keep one outcome shape. Rejected: it
    names a repair that never ran, prints advice that cannot help, and the
    user's own directory is not a convergence failure.
  - Substitute an in-process writer for the production stream choice.
    Rejected: `repair_production`'s job is to choose the process descriptor;
    hiding that behind a parameter would remove the decision under test.
  - Re-run every control D-12 and D-13 recorded to regenerate their raw logs.
    Rejected for this round: their decisive lines are already quoted in the
    decision log, the claims they support have not changed, and re-running
    them would add hours without changing a disposition.
- Approving authority: the user's instruction to take PR #603 through
  babysitting to merge.
- Date/Author: 2026-11-20, implementation agent.

### D-15: assessments round four (accepted 2026-11-20)

- **Decision.** The completeness and proofs replies to D-14
  (`EP352-COMPLETE-20261120T060853Z` and `EP352-PROOFS-20261120T060853Z`, both
  given at head `3acce0f5` against plan revision 7) name three code findings
  and one evidence finding. Each is actioned against the tree. The Windows
  import finding was already repaired in the disposition pass and is covered by
  the progress entry above, so it is not repeated here. A CodeScene duplication
  finding raised on the round-four tree is actioned here as well, and the
  Windows job's next run exposed a second, platform-specific test failure in
  `repair_flow`, repaired in the progress entry above.
- **The direct route re-checks its suite before Dylint starts.** `run_direct`
  applies `suite_rule(LibraryRoute::UserManaged, path)` before it spawns
  `cargo dylint`, so a user path that lost its suite, or gained a second one
  after readiness, is refused under `provision.user-path` instead of passing
  Dylint's `1` through as a lint result.
  `recheck::a_user_path_that_changed_before_the_run_is_refused` removes the
  approved suite and asserts the typed refusal, which is only reachable before
  the child is spawned. The bootstrapped route already had the same re-check;
  the two now share `suite_rule`.
- **A refused run under `--no-install` keeps the refusal key.** `ran` maps
  `RunFault::Provisioning(reason)` to `Refused(reason)` when repair is
  forbidden: nothing was installed, so the run re-check's finding is the defect
  readiness would have reported a moment later, and it reports the reason's own
  key with zero repairs. `provision.not-converged` stays reserved for a repair
  that ran, which is why the post-repair path still uses `classified`.
  `policy::a_run_fault_the_flow_may_not_repair_reports_its_refusal_key`
  (`recorded_no_install` case) asserts the outcome, the single run, zero
  repairs, the `whitaker: error[provision.refused]:` line and the absence of
  `provision.not-converged`.
- **The two refusal tests are one table.** CodeScene reported the no-install
  test and the D-14 user-path test as duplicated functions. They differed only
  in their route's data, so they are now the two cases of
  `a_run_fault_the_flow_may_not_repair_reports_its_refusal_key`, which takes
  the user-managed flag, the run fault, the expected outcome and the key. Every
  assertion is unchanged and every message names its case's key, so a failure
  still says which route broke. `cs check src/cli/check/tests/policy.rs` reports
  `Code health score: 10.00` with no duplication warning.
- **The earlier controls were re-run with their commands.** Six mutations were
  applied to this tree, run, and reverted. Afterwards
  `grep -rn "NEGATIVE CONTROL" src/ verus/` found nothing, and both the
  `git diff` and `git status --porcelain` SHA-256 sums matched their
  pre-control values. Kani routing and exit, Verus routing and separator, and
  the argument unit and property controls each now have a transcript naming the
  mutation, the command and the decisive lines, with the green baseline each
  was compared against.
- **Rationale.** Both code findings were real. Without the direct re-check, a
  path that changes between readiness and the run reports Dylint's `1`, which
  is indistinguishable from a lint denial; and an invocation that installed
  nothing cannot have failed to converge. The evidence finding is also real: a
  transcript that omits its command cannot be reproduced by the reader.
- **Alternatives.**
  - Amend the transcript requirement to accept the earlier records' decisive
    lines without commands. Rejected: the requirement exists so a reviewer can
    reproduce the control, and re-running six mutations costs minutes.
  - Keep the unconditional double-call at-most-once control as the current
    one. Rejected: the plan's two-part control supersedes it, and D-14 already
    ran that; the earlier record keeps its command for history rather than
    being presented as current.
  - Treat the argument controls as already satisfied by the D-12 summary.
    Rejected for the same reason as the first alternative: their record named
    failing tests but no command.
- Approving authority: the user's instruction to take PR #603 through
  babysitting to merge.
- Date/Author: 2026-11-20, implementation agent.

### D-16: assessments round five (accepted 2026-11-20)

- **Decision.** The completeness reply to D-15
  (`EP352-COMPLETE-20261120T060853Z`, given at head `c17c4264` against plan
  revision 8) clears the round-four findings by inspection and names two
  remaining items: the Windows test build now fails on a fixture that has no
  Windows consumer, and the direct route has no test for the second-suite shape
  that asserts the child never starts. Both are actioned. The proofs reply
  (`EP352-PROOFS-20261120T060853Z`, same head and revision) clears the proof
  bodies and names one evidence defect: the recorded user-path control mutated
  `classified`, which `ran`'s refusal arm now bypasses, so it cannot show F7
  still fails. That is actioned by re-running the control against `ran` and
  labelling the superseded records historical.
- **The Windows fixture is gated with its consumer.** `RepairSteps` counts an
  install, so a fixture that delegates readiness to the production probe and
  counts what it installs is only useful to the Unix-only provocation. Gating
  the test in `repair_flow` therefore left `RealReadiness` and its `new`
  compiled and unused on Windows, which `-D warnings` reports as two errors.
  The struct, both implementations and the `SystemSteps` import they alone use
  are now `#[cfg(unix)]`. The policy itself keeps a platform-neutral test,
  `a_host_fault_is_reported_without_installing`, which scripts the in-lock
  answer instead of provoking a filesystem fault whose meaning differs by
  platform.
- **The direct route's second shape is observed, not inferred.** `run_direct`
  now delegates to `run_direct_with_cargo`, which takes the program to start.
  Production passes `cargo`; the new test passes a shell stub that appends to a
  log when it starts.
  `a_user_path_that_stopped_naming_one_suite_is_refused_before_cargo_starts`
  drives both changed shapes — the suite removed, and a second suite added —
  and asserts the typed refusal for each and that the log was never written.
  The seam mirrors `preflight_recorded_with_cargo`, which already names the
  program for the same reason: a test that needs a real child must supply it
  rather than reach into the process environment.
- **The user-path control follows the arm the flow consults.** Since D-15,
  `ran` returns `Refused` for a provisioning fault that may not be repaired,
  before `classified` is called, so mutating `classified` cannot change F7's
  outcome. The control is now a mutation of that refusal arm to `NotConverged`;
  under it the `Kani` harness reports at `src/cli/check/flow/verification.rs`
  line 292 that a user path that lost its suite was reported as a repair that
  failed, and the nextest policy table fails both of its cases with
  `Check(NotConverged(..))` on the left against `Check(Refused(..))` on the
  right. The unmutated baseline for the same filter is
  `2 tests run: 2 passed, 205 skipped`. The two earlier records stay in the
  plan, labelled historical with the reason they no longer reach the decision
  they name.
- **Rationale.** The Windows failure is a compile error, not a policy change;
  the fixture's only consumer is Unix-only by design, so gating it is the
  honest fix and an `#[expect(dead_code)]` would hide a real absence of use.
  The direct-route gap matters because the missing-suite and two-suite shapes
  are judged by the same call, and a check that handled only the first would
  still let Dylint's `1` pass through as a lint denial; an error-value
  assertion alone cannot show whether a regressed check spawned first. A
  control is evidence only against the decision it mutates, so keeping a
  transcript that mutates a function the path no longer reaches would present
  the appearance of a control without its effect.
- **Alternatives.**
  - Keep the fixture compiled on Windows with an item-scoped expectation.
    Rejected: it has no Windows consumer by design, and the repository forbids
    expectations that hide unused code rather than narrowing the boundary.
  - Add the two-suite case to
    `a_user_path_that_changed_before_the_run_is_refused`
    through `SystemServices::check_run`. Rejected: it repeats the inference the
    finding asked to replace, and it would still not observe the child.
  - Test the stub through a temporary `PATH` change in the test process.
    Rejected: the repository injects the environment rather than mutating the
    process's, and the program seam is the narrow equivalent here.
  - Delete the superseded control transcripts and keep only the new ones.
    Rejected: the plan's evidence section keeps each record for the tree it
    was run on, and a historical entry with its reason shows why the earlier
    mutation stopped being decisive.
- Approving authority: the user's instruction to take PR #603 through
  babysitting to merge.
- Date/Author: 2026-11-20, implementation agent.

### D-17: the notice precedence is tested at the run boundary (accepted 2026-11-20)

- **Decision.** The thread reply to D-16's dispositions asked for a boundary
  test: a failing writer and a completed Dylint status of `1` must reach the
  caller as `1` through `run_bootstrapped` or `SystemServices::check_run`, not
  only through `completed_status`. `run_bootstrapped` now takes its three
  related inputs as a `BootstrappedRun` and the operation that starts Dylint as
  a closure parameter; `check_run` passes `run_prepared`, and
  `a_caller_keeps_the_completed_status_when_the_notice_is_refused` passes a
  completed `RunOutcome { code: 1, built_driver: true }` with a `ClosedStream`
  and asserts the returned status is `1`. The same commit groups the inputs
  because Clippy's argument-count lint forbids a fifth parameter.
- **The run path has its own module.** The added seam took
  `src/adapters/check/mod.rs` from 396 to 429 lines, past the 400-line limit
  (constraint 8, "new code goes in new modules"). The bootstrapped and direct
  runs, the notice precedence and the `cargo`-program seam moved unchanged to
  `src/adapters/check/run.rs`; `mod.rs` keeps readiness, repair and the suite
  rule, and the tests import the run items through `super::run`. The item
  visibility is `pub(super)` so the adapter and its tests can reach them
  without widening the crate surface. `docs/repository-layout.md` now names the
  run module alongside preflight, repair and telemetry.
- **The route and the suite are real.** The test obtains a genuine
  `PreparedRun` from `preflight_recorded_with_cargo` over the
  `ReadyInstallation` fixture with the suite staged in the recorded directory,
  so `suite_rule` approves the path before the injected run is consulted. Only
  the child is replaced.
- **Rationale.** The helper tests pin the precedence rule; the boundary test
  pins the promise the port makes to its caller. Without it, a regression that
  let the notice failure escape the helper would pass every existing test. The
  child cannot be stubbed without mutating the test process's `PATH`, which
  this repository forbids outside subprocess isolation, so the injected
  operation is the narrowest seam that still exercises the decision.
- **Control.** Inverting the precedence — making `completed_status` answer
  the output-failure status `5` in place of the completed run's status when the
  writer refuses the notice — was applied, run, and reverted; the file was then
  compared with its pre-control copy and matched. Command:
  `cargo nextest run --locked -p whitaker --lib -E 'test(adapters::check::tests::status)'`.
  Both halves failed, the helper and the boundary alike:
  `a_refused_notice_does_not_replace_the_lint_status` and
  `a_caller_keeps_the_completed_status_when_the_notice_is_refused` each
  panicked with `left: 5`, `right: 1`, and the run reported
  `Summary: 4 tests run: 2 passed, 2 failed, 204 skipped`. The control was
  re-run after the module move, so its record names the final tree; the file
  was restored and compared byte-for-byte with its pre-control copy, and the
  same command then reported `4 tests run: 4 passed`
  (`/tmp/nextest-control-status-final-tree.out`).
- **Alternatives.**
  - Keep only the `completed_status` tests. Rejected: that is exactly the gap
    the finding named.
  - Drive the real child from an end-to-end binary test with a stub `cargo`.
    Rejected: producing `built_driver: true` requires a child that writes the
    driver slot, and closing the test's standard error relies on signal and
    pipe behaviour rather than the writer contract under test.
  - Make the test set `PATH` for the test process. Rejected: the repository
    injects the environment rather than mutating the process's.
- Approving authority: the user's instruction to take PR #603 through
  babysitting to merge.
- Date/Author: 2026-11-20, implementation agent.

### D-18: the readiness helper is handed its host facts (accepted 2026-11-21)

- **Decision.** The completeness assessment's revision-10 reply names two
  placements that survived the earlier rounds. `preflight_recorded_with_cargo`
  read `PATH` itself, against the plan's requirement that the composition root
  reads it once and passes it down; and `CheckRunner::preflight` recorded
  telemetry, putting an observation inside the readiness port. Both are moved
  out. `SystemServices::preflight` and `SystemServices::repair` read `PATH`
  once each and hand it down as `Option<&OsStr>` through `preflight_recorded`,
  `preflight_recorded_with`, `preflight_recorded_with_cargo`,
  `preflight_user_managed`, `preflight_user_managed_in` and the repair seam's
  new `HostEnv`, so nothing below the adapter reads the environment. Readiness
  telemetry moves to `RecordedPorts`, a wrapper over `CheckPorts` that the
  command constructs, which records the class of each answer the flow consumed
  and the success of each repair.
- **The wrapper is at the command boundary, not in the flow.** The flow's
  module documentation says it is free of adapters, and the Kani harnesses
  drive it through `CheckPorts`; recording inside `run_check_flow` would put a
  side effect in the pure decision and force every harness to tolerate it. A
  decorator over the same trait leaves the flow untouched, keeps one event per
  readiness question including the one asked after a repair, and is itself
  replaced by a scripted port in tests.
- **One event per question, not one per command.** The repair route asks
  readiness twice: once to decide that a repair is warranted, once after the
  installer ran. Wrapping the ports observes both. Recording at the outcome
  would lose the second question, and recording in the adapter's `preflight` is
  the placement the finding rejected.
- **`HostEnv` groups the two facts readiness needs.** `RepairSteps::readiness`
  took only the directories; it now takes the directories and the caller's
  `PATH` as one value, which keeps the argument count within Clippy's limit
  without silencing it and makes the borrowed lifetime explicit at the seam.
- **Rationale.** The plan requires that `check` read `PATH` once at the
  composition root exactly as forwarding does, and that telemetry be emitted at
  the command boundary. Both were true of the forwarding path and drifting in
  `check`; the repairs make the two commands agree, and each is pinned by a
  test that fails when the placement regresses.
- **Round-seven correction: the reads are snapshotted for the invocation.**
  The completeness assessment's next reply pointed out that reading `PATH` once
  per service call still reads it three times in a repair flow — once before
  the repair, once inside it, once after — and that the module's "once per
  invocation" wording therefore described the code inaccurately. Rather than
  weaken the wording, the reads move to construction. `SystemServices` is now a
  struct that snapshots `PATH`, `DYLINT_LIBRARY_PATH` and `CI` in `new`, and it
  lives in its own module, `src/adapters/services.rs`, so the composition root
  is one small file. `preflight`, `repair` and `forward` read the snapshots;
  `active_library_path` takes the explicit `DYLINT_LIBRARY_PATH` as a parameter
  and `explicit_library_path` no longer reads the environment, so no helper
  below the composition root reads the process environment at all. A handwritten
  `Default` delegates to `new`, because a derived one would leave every
  snapshot `None` and silently drop the Windows `PATH` adjustment.
  `an_injected_explicit_path_precedes_recorded_and_default_paths` and
  `an_empty_explicit_setting_is_not_authoritative` drive the resolution with a
  value the test chose, replacing a test that needed a process-wide environment
  lock to do the same. Control: making `explicit_library_path` ignore its
  argument failed
  `an_injected_explicit_path_precedes_recorded_and_default_paths` with
  `left: Resolved { path: ".../nightly-2026-05-28/release", candidates: 1 }`,
  `right: Explicit("/custom/dylint/lib")`,
  `Summary: 19/20 tests run: 17 passed, 1 failed, 191 skipped`
  (`cargo nextest run --locked -p whitaker --lib -E 'test(library_path)'`,
  `/tmp/nextest-control-libpath.out`); the file was restored and compared
  byte-for-byte.
- **Round-eight correction: the shape change is a documented compatibility
  decision.** The completeness assessment's revision-12 reply noted that the
  snapshot changes the public shape of `SystemServices` — a unit struct with a
  `const fn new` becomes a struct with private fields and a run-time `new` —
  and that the plan's "the new root items are pre-1.0 and application-internal"
  covers additions, not a change to a type that already existed. The root
  `whitaker` package is published on crates.io (0.3.0, 2026-11-16), and the
  published `src/adapters/mod.rs` exports `pub struct SystemServices;` with
  `pub const fn new`. The change is kept, because no cheaper remedy satisfies
  the read-once requirement, and it is recorded as a source-compatibility
  decision in [`docs/migrations/0.4.0.md`](../migrations/0.4.0.md) rather than
  left implicit: the guide states that `new` is no longer `const`, that the
  unit value no longer exists, that `Default` now snapshots, that one adapter
  set should be built per invocation, and gives both broken shapes beside their
  replacements. A migration guide is the right place: it is what a downstream
  consumer reads before upgrading, and the published crate carries no other
  record of the previous shape.
- **Round-eight correction: the Windows `PATH` evidence is a verification
  limit.** The round-seven reply said
  `the_prepared_path_reaches_the_child_command` runs on every host. It does not:
  `installer/src/bootstrap/run.rs` declares `run_tests.rs` — which includes
  `run_path_tests.rs` — as `#[cfg(all(test, unix))]`, so that test and
  `an_unprepared_path_is_left_inherited` run only on Unix. What is tested on
  every host is the arithmetic, `an_injected_path_gains_the_toolchain_bin`,
  because `with_toolchain_bin` is `#[cfg(any(windows, test))]`; and the
  forwarding side is pinned by
  `the_callers_path_reaches_the_locked_readiness_step`. The Windows end-to-end
  effect therefore stays a verification limit: no test on this host can observe
  it, since `child_path` returns `None` off Windows. The `PATH` mechanism
  predates this branch — `child_path.rs` and `RunOptions::inherited_path` exist
  at the base — so widening its test coverage is not a change this pull request
  makes, and the qualification belongs to the evidence rather than to the diff.
- **Controls.** Both were applied to the current tree, run, and reverted; each
  file was compared with its pre-control copy and matched byte-for-byte.
  - Dropping the injected value at the repair seam — passing
    `HostEnv { dirs, inherited_path: None }` to `readiness` — failed
    `the_callers_path_reaches_the_locked_readiness_step` with `left: None`,
    `right: Some("/whitaker/injected-bin")`
    (`cargo nextest run --locked -p whitaker --lib -E 'test(repair_flow::the_callers_path_reaches)'`,
    `Summary: 1 test run: 0 passed, 1 failed, 209 skipped`;
    `/tmp/nextest-control-path.out`).
  - Silencing the wrapper's readiness recording failed
    `each_readiness_answer_reaches_the_subscriber_once` with `left: 0`,
    `right: 2`, and the captured events showed only the repair and the decision
    (`cargo nextest run --locked -p whitaker --lib -E 'test(cli::check)'`,
    `83 tests run: 82 passed, 1 failed, 127 skipped`;
    `/tmp/nextest-control-telemetry.out`).
- **Alternatives.**
  - Record readiness inside `run_check_flow`. Rejected: it puts a side effect
    in the module documented as free of adapters and forces the Kani harnesses
    to carry it.
  - Record in `SystemServices::preflight`. Rejected: that is still inside the
    readiness port, which is the placement the finding named.
  - Derive the telemetry after the fact from the returned outcome. Rejected:
    the outcome does not say how many readiness questions were asked, so the
    post-repair question could not be counted separately.
  - Keep reading `PATH` in the helper and document the exception. Rejected: the
    plan states the composition-root rule, and a second ambient read is the
    drift the rule exists to prevent.
  - Read once per service call and correct the wording to match. Rejected: a
    snapshot taken at construction is both cheaper and stronger — a command
    that runs for minutes then cannot act on two different environments — and
    the wording then needs no qualification.
  - Keep `SystemServices` a unit struct and thread the snapshot through
    `main`. Rejected: `main` would become the composition root while the type
    that implements every port stayed unaware of the values it needs, and each
    port method would take another parameter.
- Approving authority: the user's instruction to take PR #603 through
  babysitting to merge.
- Date/Author: 2026-11-21, implementation agent.

## Outcomes & retrospective

### `EP-M1` (2026-11-17)

- **Delivered.**
  - `whitaker check` parses `--no-install` and a forwarded argument vector,
    routes through the `CheckServices` port, and lints only against a
    preflight that just succeeded.
  - The installer promotes `bootstrap::preflight` and
    `bootstrap::run_prepared` with `PreparedRun` and `RunOptions`, and adds
    `with_install_lock` for `EP-M2`.
  - `CheckOutcome`, `check_exit_code` and `reason_key` implement ADR 0017;
    `check --help` prints the exit table.
  - ADR 0017 is drafted and indexed in `docs/contents.md`.
- **Evidence.**
  - `cargo nextest run -p whitaker --lib`: 127 passed.
  - `cargo nextest run -p whitaker --test behaviour_whitaker_check --test
    behaviour_whitaker`: 12 passed.
  - `cargo nextest run -p whitaker --test e2e_check_exit_codes`: 10 passed,
    after a red run in which `a_lost_help_write_is_the_check_output_failure`
    and the host case failed. The host case's red run is the discovery
    recorded under `Surprises & discoveries`; the lost-write red run showed
    exit `5` with an empty standard error, and the router change is what turns
    it green.
  - `cargo nextest run -p whitaker-installer bootstrap::run
    orchestration::lock`: 29 passed, including the six promoted-API tests.
  - `./scripts/run-verus.sh verus/whitaker_cli_dispatch.rs`: 8 verified, 0
    errors. `./scripts/run-verus.sh verus/whitaker_check_argv.rs`: 5 verified,
    0 errors.
  - The Kani `whitaker-cli` group, `make verus`, and the full gate suite are
    recorded in the pull request that lands this milestone.
- **Lessons.**
  - A status contract stated in an ADR is only observable if every path that
    produces the status also produces the reason key; the display path needed
    an explicit report, not just a mapping.
  - "Remove `HOME`" is not a portable way to make a directory unresolvable:
    platform directory crates fall back to the passwd entry.

### `EP-M2` (2026-11-18)

- **Delivered.**
  - `whitaker check` repairs a missing or incomplete installation under
    `with_install_lock`, re-preflights, and lints only if the fresh preflight
    is ready.
  - `--no-install`, a user-managed library path without the suite, a failed
    repair and a non-converging repair each exit `4` with their reason key;
    lock, data-directory and spawn failures exit `5`.
  - `whitaker.check.repairs` is emitted with an `outcome` label.
  - The installer's own output cannot reach standard output.
- **Evidence.**
  - `cargo nextest run -p whitaker --lib 'check::'`: 73 passed.
  - `cargo nextest run -p whitaker --test behaviour_check_repair`: 6 passed.
  - `cargo nextest run -p whitaker --test behaviour_whitaker_check`: 4 passed,
    after a red run in which the user-managed-path refusal failed because its
    "Dylint never started" step had no fixture to observe; the recording stub
    described under `Surprises & discoveries` is the fix.
  - `cargo nextest run -p whitaker --test e2e_check_exit_codes`: 9 passed.
  - `cargo nextest run -p whitaker --test e2e_check_streams`: 2 passed.
  - `cargo nextest run -p whitaker-installer orchestration::lock`: 8 passed,
    including `another_process_is_excluded_while_the_lock_is_held`.
  - `./scripts/run-kani.sh whitaker-cli`: 8 harnesses verified and 12 expected
    counterexamples observed; no reachability harness verified unexpectedly.
  - `make verus`: 68 verified, 0 errors.
  - The full gate suite is recorded in the pull request that lands this
    milestone.
  - H-6's two-process test arrived later, while closing the second completeness
    assessment: `cargo nextest run -p whitaker --test e2e_check_lock` reports
    1 passed. D-12 records its red run and the fixture lesson.
- **Lessons.**
  - A command that can repair changes what a test means. "Unprovisioned" now
    has to be asserted (`--no-install`, an empty data root), or the test will
    install the host it runs on.
  - The strongest guarantee about a stream is a type that cannot reach it.
    Redirecting the installer's stdout inside the adapter removed a whole
    class of regression that a review comment could only have caught once.

### `EP-M3` (2026-11-18)

- **Delivered.**
  - `docs/users-guide.md` leads Quick Setup with `whitaker check` and
    documents the forwarded-argument rules, the `--manifest-path` caveat, the
    installation lock, stream purity, `--no-install`, user-managed library
    paths, the exit-status table, the reason-key table, the CI recipe and the
    stale-bundle gap.
  - `docs/whitaker-cli-design.md` marks `check` implemented for routing, lazy
    repair, `--no-install`, forwarded arguments and the exit contract, and
    records the readiness, preflight, repair, stream-purity, metric and
    forwarded-argument design (D-6), deferring the configuration and selection
    surface to roadmap item 3.5.4.
  - `docs/developers-guide.md` documents the `CheckServices` port and its
    re-use policy, the `CheckPorts`/`run_check_flow` boundary, the Kani and
    Verus artefacts, and the installation lock with its two-process test.
  - `docs/adrs/adr-0012-whitaker-cli-boundary.md` gains an implementation note
    that does not revise the decision, and `docs/roadmap.md` narrows 3.5.2,
    marks it done, and adds 3.5.4.
  - `docs/contents.md` and `docs/repository-layout.md` index this plan,
    `src/cli/check/`, `src/adapters/check/` and the `--no-metadata` option.
- **Evidence.**
  - `make check-fmt`: 130 files unchanged.
  - `make markdownlint`: 0 errors after one red run, which rejected
    `normalised` in this plan's own `Surprises & discoveries` text.
  - `make nixie`: all diagrams validated.
  - `make lint`: exit `0`.
  - `make typecheck`: exit `0` in 1m19s; no `error:` or `warning:` line in the
    transcript.
  - `make test-doc`: exit `0` in 14m30s; 13 `test result: ok` suites and no
    failing doctest. `make test` does not run doctests, and this change adds
    public items with doc examples, so both gates are required evidence.
  - `make test`: 2803 passed, 8 skipped, plus the 134, 158 and 50-test support
    passes and the 1, 71 and 14 `dylint_testing` driver tests. This run is the
    one that first passed the re-blessed `legacy_installer_error_match.stderr`.
  - `./scripts/run-kani.sh whitaker-cli`: exit `0` in 8m5s; 9 properties
    verified and 13 expected counterexamples observed, which was the full
    harness set at this milestone. These counts are historical: the harness set
    grew afterwards, and the `Progress` entry recording the final-tree gates
    carries the current figures. The first run on the final tree was red: see
    the 2026-11-19 progress entry recording the assertion that had to change.
  - `make verus`: exit `0` in 6s over nine proof files, 68 verified, 0 errors.
    The final-tree run reports a larger proof count; see the same `Progress`
    entry.
  - The full gate transcripts are recorded in the pull request that lands this
    milestone.
- **Lessons.**
  - Writing the user-facing text is what made two unstated details explicit:
    the design document's "forwards to `cargo dylint`" had to name the exact
    vector, and its Whitaker-owned options were missing `--no-metadata`. The
    document now states the set the code assembles, and the extra option
    exists so a workspace that built its own `whitaker_suite` cannot shadow
    the recorded library.
  - Every status and key in the new tables was read back from `exit.rs` and
    `messages.rs` rather than restated from the plan. A documentation table is
    a second copy of a contract, and the copy carries no compiler.
  - `mapsplice` could not express the roadmap edit: `replace` failed closed on
    a dependency anchor even for an identical fragment, and `insert --after`
    renumbered unrelated sub-tasks. The edit was made by hand because it adds
    no sibling before an existing numbered item, so nothing needed renumbering.
    Recorded under `Surprises & discoveries`.

Record the next milestone's outcomes at its own boundary.

### Negative-control transcripts (2026-11-20)

Each entry names the mutation, the command, and the decisive output lines. A
transcript here means those lines, not a full build log: the full logs are under
`/tmp/<name>-3-5-2-whitaker-check-as-the-default-linting-path.out` for the
landing session. Every mutation was applied, run, and reverted; afterwards
`grep -rn "NEGATIVE CONTROL" src/` found nothing and `git diff --stat` matched
the pre-control list. An entry marked historical was run on the tree it names
by implication — before the refusal arm D-15 added — and is kept for that
scope, not as evidence about the current decision path.

- **Route re-check (nextest).** `run_bootstrapped` applied
  `recorded_suite_present` to every route.

  ```text
  cargo nextest run --locked --lib -E 'test(recheck)'
  12 tests run: 11 passed, 1 failed, 188 skipped
  FAIL adapters::check::tests::recheck::a_user_path_that_changed_after_readiness_keeps_the_user_path_reason
  assertion `left == right` failed: a user path that lost its suite must keep the user-path reason
        left: LibrariesMissing
       right: UserPathHasNoSuite
  ```

- **Production marker stream (nextest).** `repair_production` opened standard
  output for the installer.

  ```text
  cargo nextest run --locked --lib -E 'test(repair_streams)'
  1 test run: 0 passed, 1 failed, 199 skipped
  panicked at src/adapters/check/tests/repair_streams.rs:126:5:
  an installer marker must never reach standard output:
  running 1 test
  whitaker-installer: marker-stream-probe
  test adapters::check::tests::repair_streams::repair_production_child ... ok
  ```

- **User-path outcome (nextest), historical.** This control predates D-15's
  refusal arm: `classified` then mapped the run fault, and the test it names
  has since become a case of
  `a_run_fault_the_flow_may_not_repair_reports_its_refusal_key`. It is kept for
  the scope it was run on; the current control follows it.

  ```text
  cargo nextest run --locked --lib -E 'test(/user_path_that_changed_before_the_run|run_failure_is_classified/)'
  9 tests run: 8 passed, 1 failed, 191 skipped
  FAIL cli::check::tests::policy::a_user_path_that_changed_before_the_run_is_refused_not_repaired
  assertion `left == right` failed: a user path that lost its suite before the run must be refused
        left: Check(NotConverged(UserPath))
       right: Check(Refused(UserPath))
  ```

- **User-path outcome (nextest), at this head.** `ran`'s refusal arm reported
  `NotConverged`. The unmutated baseline for the same filter was
  `2 tests run: 2 passed, 205 skipped`.

  ```text
  cargo nextest run --locked --lib -E 'test(/a_run_fault_the_flow_may_not_repair_reports_its_refusal_key/)'
  Summary: 2 tests run: 0 passed, 2 failed, 205 skipped
  FAIL cli::check::tests::policy::a_run_fault_the_flow_may_not_repair_reports_its_refusal_key::case_1_user_path
  assertion `left == right` failed: a run fault the flow may not repair must be refused under provision.user-path
        left: Check(NotConverged(UserPath))
       right: Check(Refused(UserPath))
  FAIL cli::check::tests::policy::a_run_fault_the_flow_may_not_repair_reports_its_refusal_key::case_2_recorded_no_install
  assertion `left == right` failed: a run fault the flow may not repair must be refused under provision.refused
        left: Check(NotConverged(LibrariesMissing))
       right: Check(Refused(LibrariesMissing))
  ```

- **User-path outcome (`F7`, Kani), historical.** This control also predates the
  refusal arm: it mutated `classified`, which the current flow no longer
  consults on F7's forbidden-repair path. It is kept for its scope.

  ```text
  cargo kani --package whitaker --lib --harness verify_check_flow_refuses_a_user_path_run_fault
  SUMMARY: 1 of 376 failed (2 unreachable)
  Failed Checks: "a user path that lost its suite must be refused, not reported as a repair that failed"
   File: "src/cli/check/flow/verification.rs", line 290
  VERIFICATION:- FAILED
  ```

- **User-path outcome (`F7`, Kani), at this head.** The same `ran` mutation,
  checked by the harness.

  ```text
  cargo kani --package whitaker --lib --harness verify_check_flow_refuses_a_user_path_run_fault
  SUMMARY:
   ** 1 of 376 failed (2 unreachable)
  Failed Checks: "a user path that lost its suite must be refused, not reported as a repair that failed"
   File: "src/cli/check/flow/verification.rs", line 292, in cli::check::flow::verification::verify_check_flow_refuses_a_user_path_run_fault
  VERIFICATION:- FAILED
  ```

- **`F3` two-part second-decision mutation (Kani).** The second `decide` took
  the caller's policy, and its `Step::Repair` arm called `repair` again.

  ```text
  cargo kani --package whitaker --lib --harness verify_check_flow_repairs_at_most_once
  SUMMARY: 1 of 236 failed (1 unreachable)
  Failed Checks: "repair must run at most once"
   File: "src/cli/check/flow/verification.rs", line 231
  VERIFICATION:- FAILED
  ```

- **Second-call instrumentation (Kani).** The same mutation with a temporary
  `assert!(self.repairs < 1, …)` at the top of `ProbePort::repair`, so the
  failure names the second call rather than the budget.

  ```text
  SUMMARY: 1 of 243 failed (1 unreachable)
  Failed Checks: "the flow asked its port for a second repair"
   File: "src/cli/check/flow/verification.rs", line 126, in <ProbePort as CheckPorts>::repair
  VERIFICATION:- FAILED
  ```

- **Io classification (nextest), re-confirming D-12.** `BootstrapError::Io`
  was classified `Repairable(RepairableReason::Metadata)`.

  ```text
  cargo nextest run --locked --lib -E 'test(/every_bootstrap_error_has_a_class|a_host_fault_is_reported_without_advice/)'
  12 tests run: 10 passed, 2 failed, 188 skipped
  FAIL cli::check::classify::tests::every_bootstrap_error_has_a_class::case_09_io
  assertion `left == right` failed: wrong preflight class for Whitaker bootstrap I/O failed for lints: denied
        left: Repairable(Metadata)
       right: Host(Io)
  FAIL cli::check::tests::policy::a_host_fault_is_reported_without_advice::case_2_io
  assertion `left == right` failed: a bootstrap host fault must reach the user as the same host fault
        left: Check(NotConverged(Metadata))
       right: Check(HostFailed(Io))
  ```

The controls D-12 and D-13 recorded were re-run on this tree in D-15, so their
commands are recorded too. Each mutation was applied, run, and reverted; the
pre- and post-control `git diff` and `git status --porcelain` SHA-256 sums
matched. The Kani failure line numbers are one greater than D-13's citation
because the mutation itself adds a comment line to the file under test.

- **Routing (Kani).** `classify` routed `TokenTag::Check` to `Class::Forward`.

  ```text
  cargo kani --package whitaker --lib --harness verify_classify_is_total_and_disjoint
  SUMMARY:
   ** 1 of 362 failed (4 unreachable)
  Failed Checks: "a first significant check token must select the check subcommand"
   File: "src/cli/dispatch.rs", line 217, in cli::dispatch::verification::verify_classify_is_total_and_disjoint
  VERIFICATION:- FAILED
  ```

  The unmutated baseline for the same command was
  `0 of 361 failed (4 unreachable)` and `VERIFICATION:- SUCCESSFUL`.

- **Exit mapping (Kani).** `check_exit_code` mapped `NotConverged` to `1`.

  ```text
  cargo kani --package whitaker --lib --harness verify_check_exit_composition
  SUMMARY:
   ** 1 of 46 failed
  Failed Checks: "a provisioning failure must exit 4"
   File: "src/cli/check/exit.rs", line 240, in cli::check::exit::verification::verify_check_exit_composition
  VERIFICATION:- FAILED
  ```

  The unmutated baseline was `0 of 46 failed` and `VERIFICATION:- SUCCESSFUL`.

- **Routing (Verus).** `is_forward` lost its `tokens[0] != TokenTag::Check`
  conjunct.

  ```text
  ./scripts/run-verus.sh cli
  error: assertion failed
     --> verus/whitaker_cli_dispatch.rs:158:16
  158 |         assert(!is_forward(tokens));
  error: assertion failed
     --> verus/whitaker_cli_dispatch.rs:202:16
  202 |         assert(!is_forward(tokens));
  verification results:: 6 verified, 2 errors
  ```

  The two assertions are in `lemma_predicates_partition_the_inputs` and
  `lemma_classification_agrees_with_predicates`. The baseline group run was
  `8 verified, 0 errors` for the dispatch file and `7 verified, 0 errors` for
  the argument file.

- **Argument assembly, unit level (nextest).** `DylintOption::Lib` rendered as
  `--`, and `assemble_dylint_args` emitted the library name after it, so `Lib`
  became the two tokens the plan's non-vacuity clause names.

  ```text
  cargo nextest run -p whitaker --lib -E 'test(cli::check::argv)'
  Summary: 4 tests run: 1 passed, 3 failed, 202 skipped
  FAIL cli::check::argv::tests::every_owned_option_is_one_argument_that_is_not_a_separator::case_1_lib
  FAIL cli::check::argv::tests::the_assembled_vector_puts_the_separator_after_the_owned_options
  FAIL cli::check::argv::tests::an_empty_user_suffix_still_ends_with_the_separator
  ```

- **Argument assembly, property level (nextest).** The same mutation.

  ```text
  cargo nextest run -p whitaker --test property_check_argv
  Summary: 9 tests run: 4 passed, 5 failed, 0 skipped
  FAIL assembly_covers_every_named_input_shape::case_1_empty
  FAIL assembly_covers_every_named_input_shape::case_2_plain
  FAIL assembly_covers_every_named_input_shape::case_3_separator
  FAIL no_whitaker_option_renders_as_a_separator
  FAIL every_forwarded_argument_reaches_the_assembled_vector_unchanged
  ```

- **Verus separator lemma.** `assemble` duplicated the separator.

  ```text
  ./scripts/run-verus.sh cli
  error: assertion failed
     --> verus/whitaker_check_argv.rs:116:16
  116 |         assert(assembled =~= seq![Argument::Separator] + user);
  error: assertion failed
     --> verus/whitaker_check_argv.rs:144:16
  144 |         assert(assembled =~= seq![Argument::Separator] + user);
  error: assertion failed
     --> verus/whitaker_check_argv.rs:197:12
  197 |     assert(with_user_separator.len() == 4);
  verification results:: 4 verified, 3 errors
  ```

  The three assertions are in `lemma_assembly_starts_with_whitakers_arguments`,
  `lemma_assembly_preserves_the_user_suffix` and
  `lemma_a_user_separator_is_forwarded`; the dispatch file stayed at
  `8 verified, 0 errors`.

- **At-most-once repair, unconditional double call (Kani), superseded.**
  D-12 called `ports.repair()` twice in `repair_then_lint` and ran
  `cargo kani --package whitaker --lib --harness verify_check_flow_repairs_at_most_once`,
  which failed with `Failed Checks: "repair must run at most once"` at
  `src/cli/check/flow/verification.rs:231`. The plan's two-part control above
  supersedes it and was re-run in D-14; this entry keeps the earlier record
  with the command it used.

The command is delivered as specified: `whitaker check` is the default linting
path, with the exit contract in ADR 0017, the lock-mediated repair, and the
readiness model built on the installer's own preflight (D-3). Two design
choices are worth carrying forward.

The first is that readiness is delegated, not re-derived. Every attempt to
model readiness beside the bootstrap's `prepare` produced a second rule that
could disagree with it; routing both readiness and the run re-check through one
inspection is what finally made the two agree by construction.

The second is that the port owns its vocabulary. Letting the port name
`BootstrapError` and `PreparedRun` looked economical, but it made the command
layer's policy depend on an adapter's error shape and left the "approved run"
guarantee carried by a copyable value. A port-owned fault type with one
exhaustive translation, plus an approval number carried by the prepared value,
keeps the same behaviour with the boundary in the right place.

A third lesson came from the library-path rules. A rule stated from memory of
"what Dylint probably does" is a liability: Dylint accepts
`libwhitaker_suite@.so` because it splits at the first `@` without rejecting an
empty toolchain, and a parser that rejected it would report one suite where
Dylint finds two. Reading the dependency's source and mirroring it was cheaper
than any amount of reasoning about intent, and a copy of the rule is only safe
once a test pins the disagreeing case.

The fourth is that a host's error kinds are not portable. `ENOTDIR` is a defect
in the user's list on Unix, and Windows reports the same input as absence; both
are refusals, so the regression asserts the refusal rather than the exact kind.
The same review also found that `Path::is_dir` reports every metadata failure as
`false`, which turned a host fault into advice to reinstall.

## Context and orientation

The repository is a Cargo workspace. The parts this plan touches are described
below.

### The root `whitaker` package (`src/`)

- `src/main.rs` collects arguments, configures diagnostics, calls
  `cli::run_from`, and exits with `exit_code_for(&outcome)`.
- `src/cli/dispatch.rs` holds the pure router.
  - Its types are `TokenTag`, with eight tags (`Install`, `Ls`, `Help`,
    `Version`, `DoubleDash`, `GlobalFlag`, `DylintFlag`, `Other`);
    `Subcommand` (`Install`, `Ls`); and `Class` (`Subcommand(_)`, `Forward`,
    `Display`).
  - `classify(&[TokenTag]) -> Class` skips global flags, and the first other
    token decides.
  - Its `#[cfg(kani)] mod verification` maps `value & 7` onto the tags and
    proves totality and disjointness, plus one reachability harness per
    class.
- `src/cli/parser.rs` holds the clap model.
  - `Cli` has the fields `quiet`, `verbosity` and `command`.
  - `Command` has the variants `Install(InstallArgs)` and `Ls(ListArgs)`.
  - `extract_global_options` strips `-q`/`-v` up to the first `--`, and
    `try_parse_localized_from` accepts `OsString` arguments.
- `src/cli/runner.rs` holds `run_from`, `run_command`, `print_cli_error` and
  `print_clap_error`. The last two both yield `Outcome::OperationalFailure`,
  which is `1`. It also holds `forward_arguments` and `token_tag`.
- `src/cli/exit.rs` holds the `Outcome` type (`Success`, `Display`,
  `UsageError`, `OperationalFailure`, `Forwarded(i32)`) and `exit_code_for`.
- `src/ports.rs` holds `CliError`, the `InstallService`, `LintInventory` and
  `LintRunner` ports, and `Services`, which groups the ports.
- `src/adapters/services.rs` holds `SystemServices`, which
  `src/adapters/mod.rs` re-exports alongside its port implementations.
  - Forwarding uses the bootstrap's `i32` wrapper when a bootstrap is
    installed, and `cargo dylint` directly otherwise.
  - `child_exit_code` maps Unix signals to `128 + signal`.
- `src/adapters/library_path/mod.rs` holds `active_library_path`, which
  returns `Explicit`, `Resolved` or `NoCandidate`.
- `tests/` holds the existing tests.
  - `e2e_exit_codes.rs` runs the real binary against a fake `cargo` script
    on a replaced `PATH`.
  - `behaviour_whitaker.rs`, `behaviour_whitaker/support.rs` and
    `tests/features/whitaker_cli.feature` form the rstest-bdd suite.
    `BootstrapFixture` fakes an installed bootstrap whose `cargo dylint`
    exits `23` and records its arguments.
  - `root_install_e2e.rs` and `root_install_fixture/` cover the root install.
  - `property_arg_parity.rs` is a proptest.

### The installer library (`installer/`, published as `whitaker-installer`)

- `bootstrap::run_with_options(dirs, RunOptions, args)` is public and
  returns `Result<RunOutcome, BootstrapError>`.
  - Internally, `run_inner` calls `prepare`, the query, and then `execute`,
    the command, in `installer/src/bootstrap/run.rs` and `driver_run.rs`.
  - Both are `pub(super)`, and `PreparedRun` is `pub(super)`.
  - `BUILT_DRIVER_NOTICE` is public.
- `install_flow::bootstrap_step::run_bootstrap_with_dirs` is the `i32` wrapper
  that collapses errors to `1`. `check` must not use it.
- `orchestration::run_install_for_whitaker(&InstallArgs, stdout, stderr)` is
  the full installation flow without wrapper scripts.
  - `InstallArgs::default()` exists.
  - `skip.skip_deps` and `skip.no_update` live in `SkipFlags`.
- `bootstrap::is_installed(&dyn BaseDirs) -> bool` reports whether a
  bootstrap is recorded.
- `dirs::BaseDirs` and `SystemBaseDirs` resolve data directories. The
  `test-support` feature exposes `StubExecutor`.

### Verification tooling

- `scripts/run-kani.sh whitaker-cli` runs the function
  `run_whitaker_cli_harnesses`. It lists every harness by name, and
  reachability harnesses carry their exact assertion text. Its `*)` fallback
  routes unknown group names elsewhere, so always confirm that the harness
  names appear in the output.
- `./scripts/run-verus.sh cli` and `make verus`, which runs the `all` group.
  A new proof file must be listed in both `cli` and `all`. The script runs
  Verus through the pinned `prover-tools verus run`, and the `all` group also
  includes `verus/node_origin.rs`.
- Neither script installs a verifier. Install them once with
  `make install-kani` and `make install-verus`, which use the pinned
  `rust-prover-tools`. Without them, the scripts fail with an install hint
  rather than a proof result.
- `.github/workflows/proofs.yml` installs both verifiers through the shared
  `install-prover-tools` action. It runs `make kani` and `make verus` on pull
  requests that touch its `paths`, which cover `verus/**`, `tools/kani/**`,
  `tools/verus/**`, `tools/rust-prover-tools/**` and the run scripts, but not
  `src/**`.

### Gates

`make check-fmt`, `make typecheck`, `make lint` and `make test` must pass at
every milestone. `make test` no longer runs doctests, so `make test-doc` must
also pass whenever new public items carry doc examples. Documentation also needs
`make markdownlint`, which runs `make spelling` (note that the spelling gate
checks only tracked files), and `make nixie`. Write each gate's output with
`tee` to
`/tmp/<action>-whitaker-3-5-2-whitaker-check-as-the-default-linting-path.out`.

## Conformance basis

Upstream artefacts, at `origin/main` `40a85aa5` (2026-11-16, after the v0.3.0
release). The plan was authored against `d9eb8a09` and rebased; see
`Surprises & discoveries` for the delta:

- `docs/whitaker-cli-design.md`: §Public CLI surface, §Accessibility and
  localization requirements, §`whitaker check`, and §`whitaker install`.
- ADR 0012 (`CLI-REQ-FWD`, and the ownership split for lazy repair) and
  ADR 0008 (the configuration boundary, relevant to 3.5.4).
- ADR 0014 on `main` (`docs/adrs/adr-0014-per-lint-experimental-opt-in.md`).
  It fixes how experimental lints are enabled. Phase one only has to avoid
  contradicting it, and 3.5.4 consumes it.
- The 3.5.1 plan's obligations `EP-INV-DISPATCH`, `EP-LEM-DISPATCH` and
  `EP-INV-EXIT`, which this plan amends.
- `docs/roadmap.md` item 3.5.2.
- `AGENTS.md` and `docs/documentation-style-guide.md`.

No Terms of Reference document exists for the CLI. The design document is the
highest upstream artefact.

The design document does not number its requirements, so this plan introduces
these identifiers:

- `CHK-REQ-ROUTE`: `whitaker check` is a Whitaker subcommand.
- `CHK-REQ-LAZY`: missing dependencies or bundles are installed lazily, and
  then linting runs.
- `CHK-REQ-NOINSTALL`: `--no-install` disables lazy repair.
- `CHK-REQ-FWD`: arguments after `--` reach the underlying Cargo invocation.
- `CHK-REQ-EXIT`: lint-failure semantics are preserved, and install and
  configuration failures are distinct operational errors.
- `CHK-REQ-STREAMS`: progress and diagnostics go to standard error, and
  machine-readable output stays clean.

These are deferred to 3.5.4 under D-1: `CHK-REQ-CONFIG` (`--config`,
`install.lazy`, `[cmds.check]`), `CHK-REQ-PRESENT` (`--locale`, `--colour`,
`--progress`), `CHK-REQ-SELECT` (`--select`, `--ignore`, `--extend-select`,
`--experimental`) and `CHK-REQ-INSTALLFLAGS` (`--offline`,
`--build-from-source`).

Trace chains:

```plaintext
CHK-REQ-ROUTE     -> EP-M1 -> EP-INV-DISPATCH', EP-LEM-DISPATCH' -> scenario "check is a Whitaker command"
CHK-REQ-FWD       -> EP-M1 -> CHK-LEM-ARGV, CHK-INV-ARGV -> property_check_argv, scenario "forwarded arguments"
CHK-REQ-EXIT      -> EP-M1 -> CHK-INV-EXIT, ADR 0017 -> e2e_check_exit_codes
CHK-REQ-LAZY      -> EP-M2 -> CHK-INV-FLOW -> behaviour_check_repair, Kani verify_check_flow_*
CHK-REQ-NOINSTALL -> EP-M2 -> CHK-INV-FLOW, CHK-INV-POLICY -> scenario "--no-install refuses"
CHK-REQ-STREAMS   -> EP-M2 -> CHK-INV-STDOUT -> behaviour_check_repair, e2e_check_streams
```

## Verification plan

This item introduces four decision procedures that fail silently when wrong.
Each needs more than examples:

1. Routing must recognize `check` without disturbing legacy forwarding.
2. The flow decides whether to repair, at most once, and only when permitted.
3. The exit composition maps flow outcomes to statuses.
4. The argument assembly must deliver the user's Cargo arguments exactly.

Everything else is plumbing. That means the adapters, help and advice text, and
the mapping from repair requests to `InstallArgs`. Plumbing is verified by
unit, behavioural and end-to-end tests.

### Axioms (assumed, not verified)

- `AX-DYLINT-EXIT`: `cargo dylint` exits `0` on success, and `1` for any
  error, including lint denial, compile errors and internal failures. It exits
  `2` for its own usage errors and `101` on panic. Pinned by an `EP-M0`
  observation against Dylint 6.3.0 (`installer/Cargo.toml`).
- `AX-DYLINT-LAST`: `cargo dylint` passes every argument after its first `--`,
  in order, to `cargo check`.
- `AX-CARGO-EXT`: Cargo passes every argument after the subcommand name,
  including `--`, unchanged to the external `cargo-dylint` binary.
- `AX-CLAP-LAST`: clap 4.5 collects every token after the first `--` into a
  `last = true` positional, in order, including later `--` tokens. Repository
  logic built on this is tested against real clap.
- `AX-OS-STATUS`: a child's status is an `i32` code, or a signal on Unix. The
  existing `child_exit_code` and `bootstrap::exit_code` mappings are reused.
- `AX-PREFLIGHT`: a successful readiness check covers what it claims, and no
  more. `bootstrap::preflight` validates readable metadata, the library
  directory, the Clippy snapshot manifest and a queryable `cargo dylint`;
  `check` additionally requires that library directory to hold
  `whitaker_suite`. It does not verify the snapshot's Git objects and does not
  build the driver. The run may therefore still fail with a typed
  `BootstrapError` before, or instead of, starting Dylint, and that failure
  never surfaces as exit `1`. It spends the one permitted repair, as
  `CHK-INV-FLOW` records.
- `AX-INSTALL-IDEMPOTENT`: `run_install_for_whitaker` with
  `no_update = true` leaves a complete installation complete.

### `EP-INV-DISPATCH'`: routing stays total and disjoint with `check` (amended Kani)

- **Obligation.** For every token sequence, `classify` returns exactly one
  class.
  - A sequence whose first non-global token is `Check` yields
    `Subcommand(Check)`.
  - Every sequence without `Check` classifies exactly as the eight-tag rule
    did. This regression clause guarantees `CLI-REQ-FWD`.
- **Method.** Kani over the real `classify`, plus `rstest` cases.
- **Domain.** Nine tags, generated with `kani::any_where(|v: &u8| *v < 9)` and
  a `match` (`%` is denied, Constraint 12). Sequences have length 0 to 6, so
  the domain is `sum(9^k for k in 0..=6) = 597,871` sequences.
- **Artefacts.**
  - `src/cli/dispatch.rs` `mod verification`: add the harness
    `verify_check_is_reachable` and an eight-tag oracle,
    `classify_before_check`.
  - Register the new harness, with its exact assertion text, in
    `run_whitaker_cli_harnesses` in `scripts/run-kani.sh`.
  - Add `src/cli/**` to `.github/workflows/proofs.yml` `paths`.
- **Evidence.** In the output of `./scripts/run-kani.sh whitaker-cli`,
  `verify_classify_is_total_and_disjoint` reports `SUCCESSFUL`, and every
  reachability harness, including `verify_check_is_reachable`, reports its
  expected counterexample.
- **Non-vacuity.**
  - The reachability harnesses assert `false` and must fail.
  - Negative control: route `Check` to `Forward`. The totality harness must
    then report a counterexample whose first significant tag is `Check`.
    Record the transcript, then restore.

### `EP-LEM-DISPATCH'`: unbounded classification with `check` (amended Verus)

- **Obligation.** For every finite token sequence, exactly one of
  `is_subcommand`, `is_display` and `is_forward` holds. The subcommand class
  includes `Check`.
- **Method.** Extend `verus/whitaker_cli_dispatch.rs`: add `Check` to the
  `TokenTag` model and the subcommand guard, and add a `[Check]` witness to
  `lemma_classes_are_inhabited`.
- **Rationale.** Argument vectors have no length bound, so Kani cannot
  establish this. A stale proof would describe code that no longer exists.
- **Evidence.** `timeout 600 ./scripts/run-verus.sh cli` ends with
  `verification results:: N verified, 0 errors`, and so does `make verus`.
- **Non-vacuity.** The `Check` witness is proved before disjointness.
  Negative control: add `Check` to the forward guard as well. The disjointness
  clause must fail.

### `CHK-INV-FLOW`: the repair flow (new, exhaustive Kani)

The abstract model comes first, independent of adapters:

```plaintext
Preflight  = Ready(prepared) | Repairable(reason) | Host(fault)
Policy     = { repair_allowed: bool }
Step       = Run | Repair | Refuse(reason) | Fail(fault)

decide(Ready, _)            = Run
decide(Repairable(r), p)    = if p.repair_allowed then Repair else Refuse(r)
decide(Host(f), _)          = Fail(f)

flow(ports, p) =
  match decide(ports.preflight(), p)
    Run        => run(prepared)
    Refuse(r)  => Refused(r)
    Fail(f)    => HostFailed(f)
    Repair     => match ports.repair()                 -- lock, re-preflight, install if still needed
                    Err(e) => RepairFailed(e)
                    Ok     => match decide(ports.preflight(), { repair_allowed: false })
                                Run       => run(prepared')
                                Refuse(r) => NotConverged(r)
                                Fail(f)   => HostFailed(f)
run(prepared) = match ports.run(prepared)
                  Ok(code)                    => Linted(code)
                  Err(Provisioning(UserPath)) => Refused(UserPath)
                  Err(Provisioning(r))        => NotConverged(r)
                  Err(Spawn | Host(f))        => RunFailed
```

The model above shows the initial shape. Amended during `EP-M2`: readiness is a
snapshot, and the run is the first moment it can be contradicted. A cold driver
may reject a snapshot whose generation metadata and refs still match, and a
suite can be removed after the preflight. When `ports.run(prepared)` reports
`Err(Provisioning(_))` and the policy still permits repair, `ran` therefore
calls `repair_then_lint` once, exactly as the first `Step::Repair` does. The
budget stays at one, because `repair_then_lint` re-preflights with repair
forbidden, so its second run cannot repair again.

Amended at D-14: the run fault carries its own reason, and one reason needs a
different outcome. A `UserPath` reason can only arise on the user-managed
route, where the policy already forbids repair, so the refusal spends no
budget. Reporting it as `NotConverged` would name a repair that never ran and
could not have helped: the directory belongs to the user. It is therefore
refused under the same key and advice that route's readiness failure uses, and
the run re-check applies the route that approved the path, so a path that lost
its suite between the two steps keeps the user-path reason rather than being
described as a defect of the installed libraries.

The invariants:

- `F1` (refusal honoured): when `repair_allowed` is false, `repair` is never
  called.
- `F2` (no work when the first run succeeds): when the first preflight is
  `Ready` and that run returns a status, `repair` is never called and `run` is
  called exactly once. A `Provisioning` fault from that run spends the repair,
  as the amendment above records.
- `F3` (bounded repair): `repair` is called at most once. This is
  structural, because the second `decide` always forbids repair.
- `F4` (safety): `run` receives only a `prepared` value returned by the most
  recent preflight, and only when that preflight was `Ready`. Dylint never runs
  against an installation the flow has not just seen ready.
- `F5` (host faults never cause repair): a `Host` preflight leads to neither
  `repair` nor `run`.
- `F6` (outcome totality): every path ends in exactly one `CheckOutcome`.
- `F7` (refusal honesty, added at D-14): a run that reports `UserPath` ends as
  `Refused(UserPath)` and never as `NotConverged`, and no repair is spent.

The method is Kani over the real generic `run_check_flow<P: CheckPorts>`. The
harness's `P` returns `kani::any()` choices. The domain is finite:

- 3 preflight classes per call, with 6 repairable reasons and 2 host faults;
- 2 repair results;
- 3 run results, with a symbolic `i32` code;
- 2 policies.

The exploration is exhaustive over the answer plans the scripted port can be
given, and it runs the real generic flow rather than a model of it. It is not
exhaustive over arbitrary per-call values: `ProbePort` samples one repairable
reason and one host fault for the whole run, so a run that mixes two different
repairable reasons, or two different host faults, is outside the domain. The
flow invariants never compare reasons or faults with each other, and the fault
tables in `cli/check/tests/faults.rs` pin their distinct propagation.

One harness states a narrower property than a first reading suggests.
`verify_check_flow_host_faults_never_repair` assumes the *first* readiness
answer is a host fault, so it does not independently assert that a host result
on the second readiness check is followed by no further work. The observable
consequences are still covered over every path: a run after any non-ready check
fails `F4`'s freshness assertion, and a second repair fails `F3`'s budget.

Verus is not used for the flow. Its state space is finite and small, so Kani's
exhaustive check over that domain, run against the real code, gives a stronger
guarantee than a Verus proof of a model would. A Verus model would also add a
gap between model and code for no gain in coverage. This is a deliberate
exception to proving business logic in Verus, recorded so that reviewers can
challenge it.

Because `CheckFlow` is generic over the narrow `CheckPorts` trait, the harness
never builds `InstallArgs` or any adapter type. Mapping repair onto
`InstallArgs` is a separate unit-tested function.

Supporting tests:

- An `rstest` table that runs `decide` over all three preflight classes for
  each policy.
- A table that maps every `BootstrapError` variant to its preflight class.
- An rstest-bdd suite that runs in-process through `run_from` with fake
  ports.

Artefacts:

- `src/cli/check/flow/mod.rs`: `CheckPorts`, `decide`, `run_check_flow` and
  `#[cfg(kani)] mod verification`.
- `src/cli/check/classify.rs`: classification of `BootstrapError` into a
  preflight class.
- New harness entries in `scripts/run-kani.sh`.

Evidence: the following harnesses report `SUCCESSFUL`:

- `verify_check_flow_refusal_is_honoured`
- `verify_check_flow_does_no_work_when_ready`
- `verify_check_flow_repairs_at_most_once`
- `verify_check_flow_runs_only_fresh_ready_preflight`
- `verify_check_flow_host_faults_never_repair`
- `verify_check_flow_refuses_a_user_path_run_fault`

Non-vacuity:

- Reachability harnesses must each produce a counterexample for `Linted`
  after a repair, `Linted` after a run-time provisioning failure and its repair,
  `NotConverged`, `Refused` and `HostFailed`.
- Negative control (run 2026-11-20, D-14): let the second `decide` use the
  caller's policy, and make its `Step::Repair` arm call `repair` again. Either
  change alone leaves the repair count at one: with the forbidden policy that
  arm is unreachable, and with the caller's policy alone it still returns
  `NotConverged`. Together they made `F3` fail with
  `Failed Checks: "repair must run at most once"` at
  `src/cli/check/flow/verification.rs:231`. A second run of the same mutation
  with a temporary assertion at the top of `ProbePort::repair` named the defect
  directly: `Failed Checks: "the flow asked its port for a second repair"`.
  Both mutations were reverted and the files checked against their pre-control
  contents.
- Negative control (D-12, re-run 2026-11-20 in D-14): classify `Io` as
  repairable. The obligation belongs to the classifier tables, not to `F5`:
  `F5` builds `Preflight::Host` directly, so classification cannot reach it.
  The control must fail `every_bootstrap_error_has_a_class::case_09_io` and
  `policy::a_host_fault_is_reported_without_advice::case_2_io`; both failed with
  `Repairable(Metadata)` where `Host(Io)` was required.

### `CHK-INV-POLICY`: when repair is allowed (new, rstest)

- **Obligation.** In phase one,
  `repair_allowed = !no_install && !user_managed_path`.
  - Under a user-managed path with a bootstrap, preflight runs with the
    user's path as the override.
  - Under a user-managed path without a bootstrap, preflight inspects the
    path for `whitaker_suite@*` library files. Exactly one is required: a
    path that holds none, or more than one, is a provisioning defect. Either
    way the outcome is `Refused`, exit `4`, with reason key
    `provision.user-path`, and `cargo dylint` never starts. The ambiguity
    rule exists because Dylint refuses to choose between two matching
    libraries and exits `1`, a status indistinguishable from a lint denial.
  - The BDD feature covers both refusals: a path with no suite and a path
    with two `whitaker_suite` libraries.
  - The same rule is re-checked immediately before Dylint starts (D-14). A
    user-managed path that stops naming exactly one suite between readiness
    and the run is `Refused(UserPath)` with the same key and exit `4`, and no
    repair is spent:
    `a_run_fault_the_flow_may_not_repair_reports_its_refusal_key::case_1_user_path`
    pins the outcome, the zero repair count, the one run, and the key, and
    `verify_check_flow_refuses_a_user_path_run_fault` states `F7` over the
    flow. The same table's `case_2_recorded_no_install` row pins the
    `--no-install` mirror, whose key is `provision.refused`.
- **Method.** An `rstest` truth table over the four combinations of
  `no_install` and `user_managed_path`, the BDD scenario, and the run-time
  refusal above. These are two Booleans plus one reason, so a truth table and a
  named case are the proportionate tools.
- **Non-vacuity.** Each row asserts whether the fake repair port was called.
  Negative control: drop the `user_managed_path` term. The "user path is never
  repaired" row must fail. D-14 records the run-time control: mapping a run's
  `UserPath` fault to `NotConverged` failed with
  `left: Check(NotConverged(UserPath)), right: Check(Refused(UserPath))`.

### `CHK-INV-EXIT`: exit composition (new, exhaustive Kani)

- **Obligation**, stated over `check_exit_code(&CheckOutcome) -> i32`:
  - `Linted(code)` exits `code`, for every `i32`.
  - Every other outcome exits a value in `{3, 4, 5}`.
  - `{3, 4, 5}` is disjoint from `{0, 1}`.
  - `Refused`, `RepairFailed` and `NotConverged` exit `4`.
  - `HostFailed`, `RunFailed` and `OutputFailed` exit `5`.
  - `ConfigurationFailed` exits `3`. It is unreachable in phase one but is
    kept so that ADR 0017 is complete. Its exit witness establishes that the
    mapped arm returns `3`; it does not establish that the command can reach
    the outcome, and the plan claims no such reachability.
- **Method.** Kani with a symbolic `i32`, an `rstest` partition, and
  end-to-end tests against the real binary. The Kani harness checks the status
  composition and that every operational outcome carries *some* reason key;
  each key's exact value is pinned by the `rstest` table in `exit.rs`. The
  end-to-end tests prove that `main` does not discard the mapping.
- **Artefacts.** `src/cli/check/exit.rs` (`CheckOutcome`, `check_exit_code`,
  the Kani harness) and `tests/e2e_check_exit_codes.rs`.
  - The end-to-end tests use a provisioned-host fixture: a stub `cargo` that
    answers `dylint --version` with the version the bootstrap metadata
    records, plus `BootstrapFixture`'s metadata, snapshot and staged library.
  - Cases: the stub `cargo dylint` exits `0`, `1`, `23` and `101`, then
    `300` on Windows only and `SIGTERM` on Unix only.
  - The provisioning case: an empty data directory with `--no-install` must
    exit `4`.
  - The host case: a child environment where no data directory can be
    determined, with `HOME` and `XDG_DATA_HOME` removed on the child
    `Command`, must exit `5`.
  - The output case: `whitaker check --help` into a closed pipe must exit
    `5`. That means `check`'s exit table applies once `classify` has returned
    `Subcommand(Check)`, before parsing.
- **Non-vacuity.** A reachability counterexample for each outcome class.
  Negative control: map `NotConverged` to `1`. The disjointness assertion must
  fail.

### `CHK-LEM-ARGV` and `CHK-INV-ARGV`: forwarded arguments are preserved (new)

The model:

```plaintext
assemble(W, U) = W ++ ["--"] ++ U
first_dd(s)    = least i with s[i] = "--"
cargo_args(s)  = s[first_dd(s) + 1 ..]          (AX-DYLINT-LAST, AX-CARGO-EXT)
```

**`CHK-LEM-ARGV` (Verus).** For every `W` without `"--"` and every `U` of any
length, including a `U` that contains `"--"`:

- `first_dd(assemble(W, U)) = |W|`, and
- `cargo_args(assemble(W, U)) = U`.

This lemma is honestly a small property of the model. It is kept because it is
cheap, and because its witness, an inner `--` that survives, is the case where
a "split at the last `--`" bug would hide.

The model is deliberately abstract: every forwarded token is the payload-free
`Argument::User`, so the lemma establishes that the boundary sits after `W` and
that the recovered sequence has the same length and token classes as `U`. It
cannot distinguish two ordinary user tokens, and therefore does not establish
their bytes or their pairwise order. That evidence is `CHK-INV-ARGV`'s: a
4,096-case property through the real parser, which is executable evidence over
a generated domain, not an unbounded proof.

The design pressure it exerts is that `W` must never contain `--`, so `W` is
built from a closed `DylintOption` enum. Each variant renders to `--name` or
`--name=value` as a single token. A unit test asserts this for every variant.

**`CHK-INV-ARGV` (proptest).** For every argument vector of the form
`whitaker [globals] check [opts] -- U`, the `CheckInvocation` handed to the
runner port satisfies `cargo_args(...) = U` byte for byte (`OsString`). The
property is tested through the real path:

1. `extract_global_options`;
2. `Cli::try_parse_localized_from`;
3. `CheckArgs`;
4. `assemble_dylint_args`.

The property supplies the executable evidence:

- `U` has length 0 to 16. Its tokens are drawn from `--`, `-p`, `-q`,
  `--help`, `--all-targets`, `--message-format=json`, the empty string, strings
  with spaces, non-ASCII strings, strings starting with `-`, and non-UTF-8
  `OsString`s on Unix.
- The globals are drawn from `-q`, `-v` and `-vv`, placed before or after
  `check`.
- Fixed cases cover `check -q`, `-q check`, `check -- -q` and
  `check -- --help`.
- `check -p x` must be a usage error (exit `2`).

An end-to-end scenario through the bootstrap route then checks that the
recorded arguments of the fake `cargo dylint` end with `-- U`.

Artefacts:

- `verus/whitaker_check_argv.rs`, listed in both `cli` and `all` in
  `scripts/run-verus.sh` and in its `case` allowlist;
- `src/cli/check/argv.rs`;
- `tests/property_check_argv.rs`.

Evidence:

- Verus reports 0 errors.
- `cargo nextest run -p whitaker --test property_check_argv` passes with
  4,096 cases.

Non-vacuity:

- The proptest classifies its cases. The run fails unless at least 10%
  contain an inner `--`, at least 10% have an empty `U`, and at least 10% start
  with a dash.
- Negative control: emit `DylintOption::Lib` as two tokens, `--` and
  `whitaker_suite`, in a scratch edit. The precondition unit test and the
  property must both fail.

### `CHK-INV-STDOUT`: standard output purity (new, tests)

- **Obligation.** During `check`, every byte Whitaker writes to standard
  output comes from the Dylint child.
- **Method.** Tests only. These are properties of real streams, and there is
  no algebra to prove.
  - In-process: the BDD repair scenarios run `run_from` with a fake
    `InstallService` that writes the `suite-source` marker to the stdout it
    is given. The captured standard output must be empty, and standard error
    must contain both the marker and the repair notice.
  - Binary level, on a provisioned host: run
    `whitaker check -- --message-format=json` against a stub that emits one
    JSON line. Standard output must parse as exactly that line.
- **Residual gap.** A binary-level test of repair plus output purity needs an
  offline repair path. It is deferred to 3.5.4, after 3.7.1 adds `--offline`.
- **Non-vacuity.** Negative control: hand the repair adapter the real
  standard output. The in-process scenario must fail on the marker line.

### Phase-two obligations (for the 3.5.4 plan)

These are stated now so that phase-one interfaces are designed for them.

- `CHK-INV-CONFIG`: `repair_allowed` gains `&& install.lazy`. `--no-install`
  always wins. A configuration error exits `3` and never runs Dylint.
- `CHK-INV-SELECT-MAP`: given the installed catalogue `C` and the resolved
  set `S ⊆ C`, the invocation enables exactly `S` and disables `C \ S`. Any
  `DYLINT_RUSTFLAGS` the user set are preserved.
  - Experimental members of `S` are enabled the way ADR 0014 on `main`
    prescribes: their names, validated by
    `whitaker_common::experimental::resolve`, reach the driver as the
    `[whitaker] experimental` table. This is not a separate bundle, and it is
    not a rustc lint-level flag, because an unlisted experimental lint has no
    pass at all.
  - The configuration spelling is `[lint].experimental_lints` in
    `whitaker.toml`, which the design names but no code reads yet.
  - The selection and the experimental table must both reach a Cargo-tracked
    rebuild input. The carriers are chosen by the H-7 evidence.
  - Methods: a proptest over generated catalogues, and Kani over the real
    catalogue's subsets.
- `CHK-INV-REPAIR-REQUEST`: `--offline` and `--build-from-source` map one to
  one onto the 3.7.1 install request.

### Not verified, deliberately

- The internals of `cargo-dylint`, Cargo, clap, `ortho_config`, rustup and
  the installer's orchestration. These rest on the axioms above and on their
  own test suites.
- The wording of help and advice text. That is snapshot-tested with `insta`
  but is not an invariant. The reason keys are asserted exactly.

### How proofs and tests relate

- Verus proves two facts about models, not about the production functions:
  the routing predicates are total and disjoint for token sequences of any
  length, and the argument model recovers the user's token sequence for any
  length. The routing model is bridged to a model of `classify`, not to the
  Rust function itself, and the argument model does not distinguish two
  ordinary user tokens.
- Kani checks the real `classify` over every sequence of at most six tokens,
  the real `run_check_flow` — and therefore `decide` as the flow uses it — over
  every answer plan its scripted port accepts, and the real `check_exit_code`
  over every outcome shape with a symbolic status. The `decide` truth table
  covers its three classes under both policies directly. These domains are the
  finite ones stated above; none covers arbitrary per-call values.
- Property tests close the gap between the argument model and the real
  parser.
- Behavioural and end-to-end tests establish what no proof covers: real
  exit statuses, real streams, the bootstrap route, and the installer call.

No proof stands in for a test, and no test is presented as exhaustive.

## Plan of work

### Stage A: prototype the Dylint invocation (`EP-M0`)

This stage changes no repository files except this plan. Its experiments run in
a scratch crate under `/tmp`, against Dylint 6.3.0 and a real installed suite.
Record each hypothesis as falsified or not falsified, with a transcript, in
`Surprises & discoveries`.

- **H-1:** `cargo dylint --lib whitaker_suite -- <args>` loads exactly the
  Whitaker suite from the bootstrap's single recorded library directory.
  Libraries from the user's workspace metadata are loaded unless
  `--no-metadata` is given. Decide whether `W` includes `--no-metadata`.
- **H-2:** under a user-managed path containing two toolchains'
  `whitaker_suite`, `--lib whitaker_suite` either chooses the toolchain that
  matches or reports ambiguity. Record which. If it reports ambiguity, `check`
  refuses with `provision.user-path` and explains.
- **H-3:** `cargo dylint` exits `1` for a deny-level lint, for a compile error,
  and for `-- --no-such-flag`. It exits `2` for an unknown option before its
  `--`. This confirms `AX-DYLINT-EXIT`.
- **H-4:** promoting `prepare`/`execute` to public functions requires no
  behavioural change in the installer. `bootstrap/run.rs` tests still pass.
- **H-5:** the warm-path cost of preflight plus run equals forwarding's cost
  to within 50 ms. Measure with `hyperfine` against a prototype built from a
  scratch branch that is never pushed.
- **H-6:** a second installer process blocks on `install.lock` and then finds
  the installation ready. Check this with a two-process script against the
  prototype.
- **H-7:** with a warm Dylint target directory, changing `DYLINT_RUSTFLAGS`
  between two runs (`-A module_max_lines`, then nothing) re-runs the lint.
  - Changing the `[whitaker] experimental` table between two runs does the
    same. Test it both through a project `dylint.toml` and through Dylint's
    `DYLINT_TOML` environment variable, the candidate carrier when `check`
    supplies the table on the user's behalf.
  - This matters only for 3.5.4, but settling it now protects the phase-one
    argument design.

There is a go/no-go point. If H-1 or H-3 is falsified, revise `DylintOption`,
the axioms and the model before `EP-M1`. If H-7 is falsified, record a required
change for the 3.5.4 plan.

### Stage B: red tests and specifications

Write each test first. Run it in a focused loop and confirm that it fails for
the expected reason. Red tests are never committed in a red state. Each commit
lands at green.

1. `tests/features/whitaker_check.feature` holds the binary-level scenarios
   for the provisioned host. Bind them in `tests/behaviour_whitaker_check.rs`
   with support code in `tests/behaviour_whitaker_check/`, reusing
   `BootstrapFixture`. The expected red failure is that the stub records
   `check` as a forwarded argument.
2. `tests/features/whitaker_check_repair.feature` holds the in-process
   scenarios, bound in `tests/behaviour_check_repair.rs` with fake ports. This
   suite cannot compile until `Subcommand::Check` and `CheckPorts` exist. Its
   red stage is therefore the compile error, followed by assertion failures
   after stub implementations whose parameters are prefixed with `_`.
3. `tests/e2e_check_exit_codes.rs`. Red: the `--no-install` case exits with
   the forwarded stub's status instead of `4`.
4. `tests/property_check_argv.rs`. Red: written against an
   `assemble_dylint_args` that returns an empty `Vec`. The property fails with
   a shrunk counterexample.
5. The Verus `Check` witness, added before the guard change. It fails to
   verify.

The Kani "red" evidence is each negative control listed under
`Verification plan`, recorded in `Outcomes & retrospective`.

### Stage C: implementation

Each module stays well under 400 lines.

1. **Installer.** These changes are atomic with their callers.
   - `installer/src/bootstrap/run.rs`: make `PreparedRun` public with private
     fields. Promote `prepare` to `pub fn preflight(dirs, RunOptions) ->
     Result<PreparedRun, BootstrapError>` and `execute` to
     `pub fn run_prepared(&PreparedRun, args) -> Result<RunOutcome,
     BootstrapError>`.
   - `run_inner` composes the two, so behaviour is unchanged.
   - Re-export both from `bootstrap/mod.rs`.
   - Add `installer/src/orchestration/lock.rs` with
     `pub fn with_install_lock<T>(dirs: &dyn BaseDirs, f: impl FnOnce() -> T)
     -> Result<T, InstallerError>`, using `fs2`. Add the new variant to the
     `clone_error!` table.
   - Unit-test each function.
2. **`src/cli/dispatch.rs` and `src/cli/runner.rs`.**
   - Add `TokenTag::Check` and `Subcommand::Check`.
   - Map the literal `check` in `token_tag`.
   - Add the Kani oracle and the reachability harness.
3. **`src/cli/check/args.rs`.** Define `CheckArgs` with `no_install: bool`
   and `cargo_args: Vec<OsString>` (`last = true`), plus after-help text with
   the exit table. Add `Command::Check(CheckArgs)` in `parser.rs`.
4. **`src/cli/check/argv.rs`.** The `DylintOption` enum (closed, settled in
   `EP-M0`) and `assemble_dylint_args`.
5. **`src/cli/check/flow/mod.rs`.** `CheckPorts` (with the associated type
   `Prepared`), `Policy`, `Step`, `decide`, `run_check_flow`, and the Kani
   harnesses.
6. **`src/cli/check/classify.rs`.** `PreflightClass`, `RepairableReason` and
   `HostFault`, plus classification of `BootstrapError` into them.
7. **`src/cli/check/exit.rs`.** `CheckOutcome`, `check_exit_code`, the reason
   keys and the Kani harness. Extend `Outcome` in `src/cli/exit.rs` with
   `Check(CheckOutcome)`, so that `main` keeps a single exit path.
8. **`src/cli/check/mod.rs`.**
   `run_check_command(&CheckArgs, &dyn CheckServices, &mut CommandStreams) ->
   Outcome`.
   It derives `Policy` (`CHK-INV-POLICY`), builds the adapter that implements
   `CheckPorts`, runs the flow, and renders advice and reason keys to standard
   error.
9. **`src/ports.rs`.** Add a `CheckServices` port with three methods:
   - `preflight(&self, mode: &LibraryMode) -> Result<Prepared, BootstrapError>`;
   - `repair(&self, stderr: &mut dyn Write) -> Result<(), CliError>`;
   - `run(&self, prepared: Prepared, args: &[OsString], stderr: &mut dyn
     Write) -> Result<i32, BootstrapError>`.

   Here `Prepared` is an enum over `Bootstrap(PreparedRun)` and
   `Direct { library_path }`. `LibraryMode` is either `Recorded` or
   `UserManaged(OsString)`. Also add a `check` field on `Services`, and add
   `Check { outcome }` handling. Keep `src/ports.rs` under 400 lines, or move
   the check port into `src/ports/check.rs`.
10. **`src/adapters/check/mod.rs`, a new directory.** Implement
    `CheckServices` for `SystemServices`.
    - `preflight` calls `bootstrap::preflight`, or the read-only suite-file
      check in direct mode.
    - `repair` calls `with_install_lock` and preflights again inside the
      lock. If the installation is still not ready, it calls
      `run_install_for_whitaker` with `no_update = true`. The installer's
      stdout parameter is a writer that forwards to standard error: a second
      `io::stderr()` handle in the system adapter, or a shared buffer in
      tests (Constraint 4).
    - `run` calls `bootstrap::run_prepared`, or spawns `cargo dylint`
      directly in direct mode, and prints `BUILT_DRIVER_NOTICE` when the
      driver was built.
    - When `CI` is set and a repair runs, it warns with advice to provision
      using `whitaker install` and to run `whitaker check --no-install`.
11. **`src/adapters/check/telemetry.rs`.** Add a span `whitaker.check`, and
    events for the preflight class, the decision and the repair outcome. Add
    the counters `whitaker.check.repairs` (label `outcome`) and
    `whitaker.check.refusals` (label `reason`), with descriptions. Labels use
    the low-cardinality reason keys only.
    - A test must show that a refusal and a repair each produce a non-zero
      counter through the collection path, `AGENTS.md`'s "Observability"
      requirement. Run the flow under the `metrics-util` debugging recorder,
      which is already a root development dependency. Asserting on helpers or
      on zero counters is not enough.
    - Error text never enters telemetry labels, and neither do user paths.
12. **Wiring.**
    - `scripts/run-kani.sh`: add the new harness names and their expected
      assertion text.
    - `scripts/run-verus.sh`: add `whitaker_check_argv.rs` to `cli`, to `all`,
      and to the allowlist.
    - `.github/workflows/proofs.yml`: add `src/cli/**` to `paths`.

### Stage D: refactor, documentation and wider validation

- Re-read the touched modules against
  `docs/complexity-antipatterns-and-refactoring-strategies.md`. The suite lints
  this repository. Keep `decide` as one flat `match`, and keep `run_check_flow`
  free of nesting by matching on `Step`.
- Do the documentation work listed under `EP-M3`.

## Milestones and plateaus

### `EP-M0`: Dylint invocation semantics settled (prototype)

- Outcome: H-1 to H-7 are recorded, and `DylintOption` is fixed. Only this
  plan changes.
- Requirements: de-risks `CHK-REQ-FWD`, `CHK-REQ-EXIT` and `CHK-REQ-LAZY`.
- Acceptance: one `Surprises & discoveries` entry per hypothesis, and a
  `Decision log` entry for the final `W`.
- Conformance:
  - No requirement is discharged.
  - The design is unchanged.
  - `AX-DYLINT-EXIT` is confirmed or corrected.
- Recovery: delete the scratch crate and the scratch branch.
- Compatibility decision: none.

### `EP-M1`: `whitaker check` on a provisioned installation

- Outcome:
  - `whitaker check [-- args]` runs the suite through the typed bootstrap API
    when preflight succeeds, passes Dylint's status through, and forwards
    Cargo arguments exactly.
  - When preflight fails with a repairable error, the result is always
    `Refused` (exit `4`) with advice to run `whitaker install`, because repair
    does not exist yet.
  - The installer API promotion lands here.
- Requirements: `CHK-REQ-ROUTE`, `CHK-REQ-FWD` and `CHK-REQ-EXIT` are
  discharged.
- Acceptance:
  - The binary-level feature scenarios pass.
  - `e2e_check_exit_codes` and `property_check_argv` pass.
  - The Kani `whitaker-cli` group passes, including the new dispatch and exit
    harnesses.
  - `./scripts/run-verus.sh cli` and `make verus` pass.
  - `make check-fmt`, `make lint` and `make test` pass.
- Conformance:
  - The ADR 0012 split is preserved.
  - The `CLI-REQ-FWD` regression oracle holds.
  - The new installer functions are additive.
  - The new root items are pre-1.0 and application-internal.
  - ADR 0017 is drafted, and its text is finalized in `EP-M3`.
- Recovery: revert this milestone's commits. `check` then forwards again.
- Compatibility decision: none. The previous `whitaker check` forwarded to
  an invocation that `cargo dylint` rejects.

### `EP-M2`: lazy repair, `--no-install` and the installation lock

- Outcome:
  - On an unprovisioned machine, `whitaker check` repairs under the lock,
    preflights again, and lints.
  - A ready installation is never repaired.
  - `--no-install` and a user-managed path both refuse with exit `4`.
  - A failed or non-converging repair exits `4` with its reason key.
  - Standard output stays pure.
  - Telemetry is emitted.
- Requirements: `CHK-REQ-LAZY`, `CHK-REQ-NOINSTALL` and `CHK-REQ-STREAMS`
  are discharged.
- Acceptance:
  - The in-process repair feature passes.
  - The Kani `CHK-INV-FLOW` harnesses pass, with their negative controls
    recorded.
  - The `CHK-INV-POLICY` table passes.
  - `e2e_check_streams` (provisioned) passes.
  - The `with_install_lock` two-process test passes.
  - All gates pass.
- Conformance:
  - Repair goes only through `InstallService` behaviour (Constraint 3).
  - A user-managed path is never repaired (Constraint 5).
  - No persisted format changes, apart from the new lock file, which holds no
    data.
- Recovery: revert this milestone's commits. `EP-M1` behaviour remains.
- Compatibility decision: none.

### `EP-M3`: documentation, roadmap and completion

- Changes:
  - `docs/adrs/adr-0017-whitaker-check-exit-status-contract.md`, following
    the style guide's ADR template.
  - `docs/users-guide.md`:
    - "Quick Setup" leads with `whitaker check`.
    - "Root CLI" covers `check`, `--no-install`, forwarded arguments, the
      `--manifest-path` caveat, the exit table, the reason keys, the CI
      recipe (a cached `whitaker install` step, then
      `whitaker check --no-install`), and the stale-bundle gap.
  - `docs/whitaker-cli-design.md`:
    - The status table now shows `check` as implemented.
    - §`whitaker check` gains the readiness, preflight and repair design, the
      exit contract with a reference to ADR 0017, and the resolution of the
      forwarded-argument wording (D-6).
  - `docs/developers-guide.md`, "Root `whitaker` CLI boundary": the
    `CheckServices` port and its re-use policy.
    - Only `check` consumes it. A later `doctor` may consume the preflight
      query.
    - Adapters compose installer queries and never install, except through
      the repair method under the lock.
    - Also document `CheckPorts` and the Kani and Verus artefacts.
  - `docs/adrs/adr-0012-whitaker-cli-boundary.md`: add a note that lazy repair
    is implemented, with a link to ADR 0017.
  - `docs/roadmap.md`: apply D-1. Narrow 3.5.2, set its `Requires` to 3.5.1,
    add 3.5.4, then mark 3.5.2 done. Applied by hand because `mapsplice` could
    not express the edit; see `Surprises & discoveries`.
  - `docs/contents.md` and `docs/repository-layout.md`: add `src/cli/check/`,
    `src/adapters/check/`, the new `verus/` file, ADR 0017 and this plan.
- Acceptance: `make markdownlint`, `make nixie`, `make check-fmt`,
  `make lint` and `make test` pass.
- Conformance: every `CHK-REQ-*` in this item is discharged, and every
  discovery is reconciled with the design document and the ADRs.
- Recovery: documentation-only commits.

## Interfaces and dependencies

There are no new dependencies. The required phase-one signatures follow. Names
may be refined during implementation, provided the change is recorded in
`Decision log`.

```rust
// installer/src/bootstrap/run.rs (promoted; behaviour unchanged)
pub struct PreparedRun { /* private fields */ }
pub fn preflight(dirs: &dyn BaseDirs, options: RunOptions<'_>) -> Result<PreparedRun, BootstrapError>;
pub fn run_prepared(prepared: &PreparedRun, args: &[OsString]) -> Result<RunOutcome, BootstrapError>;

// installer/src/orchestration/lock.rs (new)
pub fn with_install_lock<T>(dirs: &dyn BaseDirs, f: impl FnOnce() -> T) -> Result<T, InstallerError>;

// src/cli/check/args.rs
#[derive(Debug, Clone, PartialEq, Eq, clap::Args)]
pub struct CheckArgs {
    /// Refuse to install missing or broken Whitaker components.
    #[arg(long)]
    pub no_install: bool,
    /// Arguments passed unchanged to `cargo check`.
    #[arg(last = true, value_name = "CARGO_ARGS")]
    pub cargo_args: Vec<std::ffi::OsString>,
}

// src/cli/check/flow/mod.rs
pub trait CheckPorts {
    type Prepared;
    fn preflight(&mut self) -> Preflight<Self::Prepared>;
    fn repair(&mut self) -> Result<(), RepairFault>;
    fn run(&mut self, prepared: Self::Prepared) -> Result<i32, RunFault>;
}
pub enum Preflight<P> { Ready(P), Repairable(RepairableReason), Host(HostFault) }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy { pub repair_allowed: bool }
#[must_use]
pub fn run_check_flow<P: CheckPorts>(ports: &mut P, policy: Policy) -> CheckOutcome;

// src/cli/check/exit.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckOutcome {
    Linted(i32),
    ConfigurationFailed,
    Refused(RepairableReason),
    RepairFailed,
    NotConverged(RepairableReason),
    HostFailed(HostFault),
    RunFailed,
    OutputFailed,
}
#[must_use]
pub const fn check_exit_code(outcome: &CheckOutcome) -> i32;
#[must_use]
pub const fn reason_key(outcome: &CheckOutcome) -> Option<&'static str>;
```

## Concrete steps

Run every command from the worktree root. Use the shared Cargo cache. A queued
build prints `[build-limits] ... waiting for a compile slot`. That is normal
queueing, and the build must not be interrupted.

Install the pinned verifiers once per machine. Both commands are idempotent:

```bash
make install-kani
make install-verus
```

Then run the gates. The transcript is kept either way: `pipefail` makes a
failed gate fail the pipeline instead of being hidden by a successful `tee`, and
`set -e` stops the sequence at the first failure.

```bash
set -euo pipefail
BRANCH=$(git branch --show-current)
make check-fmt 2>&1 | tee "/tmp/check-fmt-whitaker-${BRANCH}.out"
make typecheck 2>&1 | tee "/tmp/typecheck-whitaker-${BRANCH}.out"
make lint      2>&1 | tee "/tmp/lint-whitaker-${BRANCH}.out"
make test      2>&1 | tee "/tmp/test-whitaker-${BRANCH}.out"
make test-doc  2>&1 | tee "/tmp/test-doc-whitaker-${BRANCH}.out"
./scripts/run-kani.sh whitaker-cli 2>&1 | tee "/tmp/kani-whitaker-${BRANCH}.out"
timeout 600 ./scripts/run-verus.sh cli 2>&1 | tee "/tmp/verus-cli-whitaker-${BRANCH}.out"
make verus        2>&1 | tee "/tmp/verus-whitaker-${BRANCH}.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-whitaker-${BRANCH}.out"
make nixie        2>&1 | tee "/tmp/nixie-whitaker-${BRANCH}.out"
```

Use these focused loops during Red-Green-Refactor:

```bash
cargo nextest run -p whitaker --test behaviour_whitaker_check
cargo nextest run -p whitaker --test behaviour_check_repair
cargo nextest run -p whitaker --test e2e_check_exit_codes
cargo nextest run -p whitaker --test property_check_argv
cargo nextest run -p whitaker --lib check
cargo nextest run -p whitaker-installer bootstrap::run orchestration::lock
```

The Kani tail should look like this (abridged):

```plaintext
Checking harness verification::verify_classify_is_total_and_disjoint...
VERIFICATION:- SUCCESSFUL
Checking harness verification::verify_check_flow_repairs_at_most_once...
VERIFICATION:- SUCCESSFUL
expected counterexample found: verify_check_is_reachable
```

Verus runs through `prover-tools verus run`. A passing run exits `0`, and each
proof file's Verus summary reports `0 errors`, for example:

```plaintext
verification results:: N verified, 0 errors
```

Treat the exit status as authoritative. The summary line comes from Verus
itself, but the wrapper may add its own framing.

Gate every commit with `make check-fmt`, `make typecheck`, `make lint` and
`make test`, plus `make test-doc` when doc examples change. Commits that touch
`src/cli/`, `installer/src/bootstrap/` or `verus/` also need the Kani and Verus
commands. Delegate full gate runs to the `scrutineer` agent.

## Validation and acceptance

### Behaviour to observe

Use an isolated `HOME`, `XDG_DATA_HOME` and `CARGO_HOME`, and a scratch crate.
The wording below is illustrative and is snapshot-tested during implementation.
The statuses, reason keys and stream separation are the contract.

```console
$ whitaker check --no-install; echo "status=$?"
whitaker: error[provision.refused]: Whitaker is not installed (bootstrap metadata missing)
help: run `whitaker install`, or rerun without `--no-install`
status=4

$ whitaker check; echo "status=$?"
[stderr] whitaker: installing missing Whitaker components (runs once)...
[stderr] ...installer progress, including the suite-source marker...
[stdout/stderr from cargo dylint]
status=0            # or Dylint's own status, unchanged

$ whitaker check -- --message-format=json | jq -c .reason | sort -u
"build-finished"
"compiler-message"
```

### Red-Green-Refactor evidence to record

For `EP-M1` and `EP-M2`, record in `Outcomes & retrospective`:

- the red and green transcripts of the feature and end-to-end tests;
- each Kani and Verus negative-control transcript.

A transcript is the mutation, the command, and the decisive output lines
recorded inline; full build logs live under `/tmp` and are not expected to
outlive the landing session. A summary that only names a `/tmp` path does not
meet this requirement, because a reviewer cannot read that path.

### Quality criteria

- `make check-fmt`, `make typecheck`, `make lint` and `make test` pass at every
  milestone, and `make test-doc` passes when new public items carry doc
  examples.
- The Kani `whitaker-cli` group and `make verus` pass at `EP-M1` and `EP-M2`.
- `make markdownlint` and `make nixie` pass at `EP-M3`.

## Idempotence and recovery

- Every step can be re-run safely.
- Tests use temporary directories and set environment variables on spawned
  children only.
- If a gate fails, fix the cause and re-run that gate, not unrelated ones.
- Milestones are separate commit ranges, so `git revert` of a range is the
  rollback.
- `EP-M0` writes only under `/tmp` and to a never-pushed scratch branch.

## Artefacts and notes

### Binary-level feature (`tests/features/whitaker_check.feature`)

```gherkin
Feature: Running Whitaker lints with whitaker check

  Scenario: check is a Whitaker command, not a forwarded invocation
    Given the whitaker binary is available
    When I run whitaker with "check --help"
    Then the command succeeds
    And the output describes the check subcommand and its exit codes

  Scenario: Dylint status passes through unchanged
    Given a provisioned Whitaker installation
    And a fake cargo dylint command returning exit code 23
    When I run whitaker with "check"
    Then the exit code is 23

  Scenario: Forwarded arguments reach cargo check unchanged
    Given a provisioned Whitaker installation
    And a fake cargo dylint command that records its arguments
    When I run whitaker with "check -- -p my-crate -- --nested"
    Then the recorded arguments end with "-- -p my-crate -- --nested"

  Scenario: An unprovisioned machine is refused under --no-install
    Given an empty Whitaker data directory
    When I run whitaker with "check --no-install"
    Then the exit code is 4
    And stderr contains the reason key "provision.refused"
    And stderr advises running "whitaker install"

  Scenario: A host fault is not a provisioning failure
    Given an environment with no determinable data directory
    When I run whitaker with "check --no-install"
    Then the exit code is 5
    And stderr contains the reason key "host.data-dir"

  Scenario: Machine-readable cargo output is not polluted
    Given a provisioned Whitaker installation
    And a fake cargo dylint command that prints one JSON line
    When I run whitaker with "check -- --message-format=json"
    Then stdout contains only the JSON line

  Scenario: Legacy forwarding is unchanged
    Given the whitaker binary is available
    And a fake cargo dylint command returning exit code 23
    When I run whitaker with "--all -- -p whitaker-common"
    Then the invocation reaches cargo dylint
    And the exit code is 23
```

### In-process feature (`tests/features/whitaker_check_repair.feature`)

```gherkin
Feature: Lazy repair in whitaker check

  Scenario: Missing components are repaired once before linting
    Given preflight reports "NotInstalled" and then "Ready"
    And the repair succeeds
    When check runs with default options
    Then the repair was requested once
    And Dylint ran once
    And the outcome is Linted

  Scenario: A ready installation is not repaired
    Given preflight reports "Ready"
    When check runs with default options
    Then the repair was not requested

  Scenario: --no-install refuses a repairable installation
    Given preflight reports "LibrariesMissing"
    When check runs with "--no-install"
    Then the repair was not requested
    And the exit code is 4 with reason key "provision.refused"

  Scenario: A user-managed library path is never repaired
    Given a user-managed library path
    And preflight reports "DylintVersion"
    When check runs with default options
    Then the repair was not requested
    And the exit code is 4

  Scenario: A repair that does not converge is reported
    Given preflight reports "NotInstalled" and then "Snapshot"
    And the repair succeeds
    When check runs with default options
    Then Dylint was not run
    And the exit code is 4 with reason key "provision.not-converged"

  Scenario: A failed repair is an operational error
    Given preflight reports "NotInstalled"
    And the repair fails
    When check runs with default options
    Then Dylint was not run
    And the exit code is 4 with reason key "provision.repair-failed"

  Scenario: Repair output stays off standard output
    Given preflight reports "NotInstalled" and then "Ready"
    And the repair writes the suite-source marker to its stdout
    When check runs with default options
    Then captured stdout is empty
    And captured stderr contains the suite-source marker
```

### Verus proof skeleton (`verus/whitaker_check_argv.rs`)

Statements only. Proof bodies are written at `EP-M1`, and the witness comes
first.

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

/// Index of the first `--`, defined recursively at EP-M1.
pub open spec fn first_dd(s: Seq<Token>) -> int;

pub open spec fn cargo_args(s: Seq<Token>) -> Seq<Token> {
    s.subrange(first_dd(s) + 1, s.len() as int)
}

/// Witness: a user suffix containing `--` survives whole.
proof fn lemma_inner_double_dash_is_preserved()
    ensures
        cargo_args(assemble(seq![Token::Word(1)],
                            seq![Token::Word(2), Token::DoubleDash, Token::Word(3)]))
            == seq![Token::Word(2), Token::DoubleDash, Token::Word(3)];

/// Whitaker's options never contain `--`, so Cargo receives the suffix whole.
proof fn lemma_cargo_args_recover_user_suffix(w: Seq<Token>, u: Seq<Token>)
    requires has_no_dd(w),
    ensures
        first_dd(assemble(w, u)) == w.len(),
        cargo_args(assemble(w, u)) == u;

} // verus!
```

The main lemma probably needs an auxiliary lemma that relates the `first_dd` of
a concatenation to its prefix. That is a hypothesis about the proof's
structure, deferred until the verification conditions are visible.

**As built.** The shipped file keeps this shape: `Argument` replaces `Token`,
`first_separator` is defined recursively, and `cargo_args` follows it. The
auxiliary lemma the sketch anticipated is the induction itself — each lemma's
body recurses on `w` after discharging its first element — so no separate
helper was needed. `has_no_separator` is used both as the lemmas' precondition
and, for the concrete two-option vector, as a proved fact
(`lemma_nothing_before_the_separator_is_a_separator`) so the general lemmas
apply without an unproved assumption.

## Signposts

Read these documents before starting:

- `docs/whitaker-cli-design.md`: the specification. §Public CLI surface and
  §`whitaker check` are normative.
- `docs/roadmap.md`: items 3.5.x to 3.9.x, which show what belongs here.
- `docs/adrs/adr-0012-whitaker-cli-boundary.md`: `CLI-REQ-FWD` and the
  ownership split for lazy repair.
- `docs/adrs/adr-0008-ortho-config-cli-boundary.md`: the configuration
  boundary 3.5.4 builds on.
- `docs/execplans/3-5-1-root-whitaker-binary.md`: the amended obligations,
  plus lessons about Kani group routing and Verus run time.
- `docs/developers-guide.md`: the root CLI boundary, Kani conventions and the
  Verus trust boundary.
- `docs/users-guide.md`: the "Root CLI" and "Planned Whitaker CLI
  configuration" sections.
- `docs/whitaker-dylint-suite-design.md`: the suite registry and experimental
  features, which matter for bundle names.
- `docs/whitaker-clone-detector-design.md`: naming conventions, so that new
  modules do not collide.
- `docs/rust-testing-with-rstest-fixtures.md`: fixture patterns for the BDD
  worlds.
- `docs/rstest-bdd-users-guide.md`: `#[scenario]` bindings and step
  functions.
- `docs/rust-doctest-dry-guide.md`: doctests on the new public items.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`: keeping
  `decide` and the flow flat.
- `docs/documentation-style-guide.md`: the ADR template, spelling and
  wrapping.
- `AGENTS.md`: gates, commits, the 400-line limit and the test-environment
  rules.

Skills to load:

- `leta` for navigation.
- `hexagonal-architecture` for the ports.
- `rust-router`, then `rust-errors` and `domain-cli-and-daemons`.
- `rust-unit-testing` for the `rstest`, `googletest`, `pretty_assertions` and
  `insta` style.
- `kani`, `verus` and `proptest`.
- `arch-decision-records` for ADR 0017.
- `mapsplice` for the roadmap edit.
- `execplans` for keeping this plan current.

## Revision note

- 2026-11-11, initial draft: written from reconnaissance and external
  research.
- 2026-11-11, revision 1, after a three-reviewer expert design review:
  - Readiness is now the bootstrap's typed preflight rather than a parallel
    model. The first draft's runner would have collapsed provisioning faults
    into exit `1`.
  - Repair is coarse, runs under a new installation lock, and never upgrades
    the managed clone.
  - Host faults now exit `5`, and every operational failure carries a reason
    key.
  - The `--manifest-path` and forwarded-argument boundary is documented.
  - Hypothesis H-7 (cache invalidation) is added.
  - The Kani, Verus and CI wiring is made explicit.
  - The test fixtures are made feasible. Repair is tested in-process, and the
    binary-level repair test waits for 3.7.1's offline mode.
  - Observability is added.
  - D-1 now proposes splitting phase two into a new roadmap item, 3.5.4,
    rather than keeping two phases inside 3.5.2.
  - Remaining work: user approval of D-1 and D-2, then `EP-M0`.
- 2026-11-16, revision 2: the branch was rebased onto `origin/main`
  `40a85aa5` (14 new commits), and the plan reconciled with them.
  - The exit-contract ADR moved from 0014 to 0016, because `main` now holds
    ADR 0013 and ADR 0014, and an unmerged branch holds 0015.
  - The phase-two notes now follow `main`'s per-lint experimental opt-in
    (ADR 0014 on `main`). There is no experimental bundle, and names reach the
    driver as the `[whitaker] experimental` table. H-7 now also tests that
    table as a rebuild input.
  - Verifier installation (`make install-kani`, `make install-verus`) is
    added, following the move to `rust-prover-tools`.
  - `make typecheck` and `make test-doc` are added to the gates.
  - The installer line references were refreshed.
  - None of this changes the phase-one design. D-1 and D-2 still await
    approval.
- 2026-11-17, revision 3: the user accepted the plan, including D-1 and D-2.
  The status is now `APPROVED`, and implementation is on hold until the user
  explicitly starts it.
  - The constraints were aligned with the current `AGENTS.md`: environment
    reads happen only at the composition root, and in-process environment
    tests use the `whitaker_common::test_support` helpers. Telemetry must be
    shown to produce a non-zero signal through its collection path.
  - Every `assert!` family call carries a diagnostic message (`AGENTS.md`,
    Rust guidance). The plan already uses `googletest` and `pretty_assertions`
    for rich failures.
  - A pre-implementation rebase and reconciliation step is added to
    `Progress`.
- 2026-11-17, revision 4: implementation began.
  - Rebased onto `origin/main` `45a436bc`. The exit-contract ADR moved from
    0016 to 0017 because `main` took 0016.
  - `EP-M0` is complete. H-1, H-2, H-3, H-4, H-5 and H-7 are recorded; H-6
    moves to `EP-M2` because the lock it races on does not exist yet.
  - D-8 fixes `W` as `--lib=whitaker_suite --no-metadata`.
  - H-2's ambiguity branch is adopted: a user-managed library path holding more
    than one `whitaker_suite` is refused rather than passed to Dylint.
- 2026-11-20, revision 5: the second completeness assessment
  (`EP352-COMPLETE-20261120T060853Z`) is dispositioned by D-12.
  - D-11's "readiness and the run re-check agree" premise is marked
    superseded: Dylint 6.3.0 cannot run a directory naming two suites, so both
    steps now apply the exactly-one rule.
  - The user-managed route reports `host.data-dir` for an unknown data
    directory instead of running the user's path.
  - The CI decision and the repair streams are seams, and the observability
    warning about discarded causes is answered with a logged cause and
    explicit counter semantics.
  - H-6's two-process test and five negative-control transcripts are recorded.
  - `docs/users-guide.md` and `docs/whitaker-cli-design.md` state the
    exactly-one rule and the new diagnostic.
  - No observable contract beyond the new refusal was added.
- 2026-11-20, revision 6: the proofs assessment
  (`EP352-PROOFS-20261120T060853Z`, given for revision 4 at head `aeefeada`) is
  dispositioned by D-13 at head `26524e55`.
  - Kani routing, Verus routing and Kani exit-mapping controls are recorded.
  - The flow exploration is described as exhaustive over the scripted port's
    answer plans, not as a complete check of the real code.
  - The argument, exit and routing claims state what their models and
    harnesses establish.
- 2026-11-20, revision 7: the third assessment round
  (`EP352-COMPLETE-20261120T060853Z` and `EP352-PROOFS-20261120T060853Z`, both
  given at head `175c7e04`) is dispositioned by D-14.
  - The flow's run arm now reports `Refused(UserPath)` for a user-path run
    fault; `F7` states the obligation and the CHK-INV-POLICY section records
    the run re-check.
  - The two-part `F3` control is run and recorded; D-12's inference is amended
    where the combined mutation contradicts it.
  - The Io non-vacuity clause names the classifier tables instead of `F5`.
  - Control transcripts are inline, and the evidence section defines what a
    transcript must contain.
- 2026-11-20, revision 8: the fourth assessment round
  (`EP352-COMPLETE-20261120T060853Z` and `EP352-PROOFS-20261120T060853Z`, both
  given at head `3acce0f5`) is dispositioned by D-15, and the CodeScene
  duplication finding raised on that tree is answered in the same round.
  - The direct user-managed route re-checks its suite before Dylint starts.
  - A run-time provisioning fault under `--no-install` is refused under its
    readiness key instead of being reported as non-convergence.
  - The earlier control records carry their mutation, command and decisive
    lines, re-run on this tree.
  - The two refusal tests are the cases of one `rstest` table.
- 2026-11-20, revision 9: the fifth assessment round
  (`EP352-COMPLETE-20261120T060853Z` and `EP352-PROOFS-20261120T060853Z`, both
  given at head `c17c4264`) is dispositioned by D-16.
  - The fixture the round-four repair left without a Windows consumer is gated
    with that consumer.
  - The direct route's second-suite case is tested against a recording stub, so
    the child's absence is observed rather than inferred from the error.
  - The user-path control is re-run against the arm the current flow consults,
    and the two records that mutated `classified` are marked historical.
- 2026-11-20, revision 10: the completeness assessment's thread reply on
  `docs/whitaker-cli-design.md` asked for the notice precedence to be pinned at
  the run boundary rather than only on `completed_status`. D-17 records the
  decision.
  - `run_bootstrapped` takes its inputs as a `BootstrappedRun` and the
    operation that starts Dylint as a parameter; `check_run` passes
    `run_prepared`.
  - `a_caller_keeps_the_completed_status_when_the_notice_is_refused` presents a
    completed `1` with a `ClosedStream` and receives `1` from the boundary,
    over a real `PreparedRun` and the recorded suite rule.
  - The run path moved to `src/adapters/check/run.rs` at the 400-line budget,
    and `docs/repository-layout.md` names the module.
  - Inverting the precedence fails both that test and the helper test with
    `left: 5`, `right: 1`; the transcript is in D-17.
- 2026-11-21, revision 11: the completeness assessment's revision-10 reply
  names two placements that survived the earlier rounds. D-18 records the
  decision and both controls.
  - `PATH` is read once in `SystemServices::preflight` and
    `SystemServices::repair` and handed down as a value through the readiness
    helpers and the repair seam's `HostEnv`;
    `preflight_recorded_with_cargo` no longer reads the environment.
  - Readiness and repair telemetry move from `CheckRunner`'s port methods to
    `RecordedPorts`, a wrapper over `CheckPorts` at the command boundary, so
    the flow stays free of adapters and each readiness question is still
    recorded once.
  - `the_callers_path_reaches_the_locked_readiness_step` and
    `each_readiness_answer_reaches_the_subscriber_once` pin both placements;
    each fails under its control.
- 2026-11-21, revision 12: the completeness assessment's revision-11 reply
  observed that reading `PATH` once per service call is three reads in a repair
  flow, so the module's "once per invocation" wording was inaccurate. D-18's
  round-seven correction records the answer: `SystemServices` snapshots the
  environment when it is built.
  - The composition root moves to `src/adapters/services.rs` and holds the
    `PATH`, `DYLINT_LIBRARY_PATH` and `CI` snapshots;
    `src/adapters/mod.rs` keeps the port implementations.
  - `active_library_path` and `explicit_library_path` take the explicit
    setting as a parameter, so no helper reads the environment;
    `an_injected_explicit_path_precedes_recorded_and_default_paths` and
    `an_empty_explicit_setting_is_not_authoritative` replace the
    `with_env_var` test.
  - `Default` is handwritten to delegate to `new`, so a `Default` adapter set
    cannot silently lose the snapshots.
- 2026-11-21, revision 13: the completeness assessment's revision-12 reply
  asked for a disposition on the `SystemServices` source-compatibility change
  and for the Windows `PATH` evidence to stay qualified. D-18's round-eight
  correction records both.
  - `SystemServices` was a unit struct with `const fn new` at the comparison
    base, and the published `whitaker` 0.3.0 crate on crates.io carries that
    shape, so the snapshot change is a source-breaking API change rather than
    an internal addition. It is kept and documented in
    `docs/migrations/0.4.0.md`, with the two shapes that stop compiling beside
    their replacements, and a `SystemServices` entry in the migration
    checklist.
  - The Windows `PATH` evidence is qualified, not extended. No test on this
    host can observe the snapshot's effect, because `child_path` returns
    `None` off Windows. The forwarding side is pinned by
    `the_callers_path_reaches_the_locked_readiness_step`; the arithmetic is
    pinned on every host by `an_injected_path_gains_the_toolchain_bin`; the
    installer's `PATH`-application tests, including
    `the_prepared_path_reaches_the_child_command`, sit in a
    `#[cfg(all(test, unix))]` module, so the Windows end-to-end effect remains
    a verification limit rather than tested coverage.
  - Only documentation changes accompany this revision:
    `docs/migrations/0.4.0.md`, this plan and `docs/developers-guide.md`. The
    Markdown gates re-run on the new tree; the Rust gates' results are not
    affected by prose, and hosted continuous integration re-runs the whole
    suite on the published commit.
- 2026-11-21, revision 14: formal review `6002879947` (eleven inline findings
  and one outside-diff finding) is dispositioned.
  - The library-path model is Dylint 6.3.0's own: entries must be absolute
    paths naming existing directories, discovery is by file name, identical
    discovered libraries count once, and the one suite-named candidate must be
    a regular file. `SuitePresence::Unusable` carries the refusal to
    `CheckPreflightError::{UserPathUnusable, RecordedPathUnusable}` — a
    user-path repair and a metadata repair — instead of the old skip or the
    old "absent" verdict. The plan's earlier tolerance of a missing directory
    is therefore reversed, and `a_missing_directory_is_refused_rather_than_skipped`
    records the reversal.
  - `BootstrapMetadata::read` reads the record as bytes and parses the slice,
    so non-UTF-8 content is malformed metadata rather than an I/O fault, which
    keeps the advice (reinstall versus investigate the host) correct.
  - `run_install_for_whitaker` delegates to `run_install_locked`, so a dry run
    never creates `install.lock`; the root behaviour scenario asserts it
    against an isolated data home.
  - `tests/e2e_check_lock.rs` proves contention with a descriptor handshake —
    the check holds an open descriptor on the lock before the holder publishes
    the record — bounded at 30 seconds, instead of inferring it from the
    child's total runtime. It reads `/proc`, so it is Linux-gated; no hosted
    macOS test lane runs it.
  - Documentation corrections: the `provision.user-path` scope in
    `docs/migrations/0.4.0.md` and `docs/users-guide.md`, the removal of the
    offline guarantee, the repair prohibition under a user-set
    `DYLINT_LIBRARY_PATH` in the users' guide and CLI design, and the
    `InstallLock` variant removed from the 0.3.0 migration guide because it
    first appears in 0.4.0.
  - `EP-M3`'s gate figures are marked historical; the final-tree figures stay
    under `Progress`.
- 2026-11-22, revision 15: the candidate-bound review on `db022402` (review
  `6005072665`), its two inline findings, and the Domain Architecture and
  Observability pre-merge rows are addressed.
  - The Dylint library-path rules are pure policy in `src/library_path/`:
    `classify` judges `EntryReport`s and performs no I/O. The filesystem read
    is a read-only seam in `src/adapters/library_path/listing.rs`, which
    classifies each entry once; the resolver's `classify_candidate` asks the
    same seam, so "missing" and "could not be inspected" cannot disagree
    between readiness and forwarding. `DirectLibraryPath::new` takes the
    classification and is the only constructor of the validated value.
  - `Path::is_dir()` is gone from the entry classification. A missing entry,
    or a successful metadata read naming a non-directory, is `Unusable`; every
    other metadata failure is an `InspectFault` and reaches `check` as
    `BootstrapError::Io` (`host.io`, exit `5`).
    `an_entry_beneath_an_inaccessible_ancestor_is_a_host_fault` holds the
    regression, skipping where the host cannot express the refusal.
  - The H-6 handshake now observes contention itself: the parent waits for
    `/proc/locks` to report the check as a blocked (`->`) waiter on the lock
    file's inode before releasing the holder, rather than treating an open
    descriptor as proof of blocking.
  - `whitaker check` counts operational failures as
    `whitaker.check.outcomes` by bounded decision label, reports every
    non-`linted` decision at `warn`, and records repair duration as
    `elapsed_ms` on the `repair completed` event and `repair_ms` on the
    `whitaker.check` span.
  - `docs/users-guide.md` limits the exit-4 claim to the file-name and
    regular-file checks and states that a corrupt or incompatible suite
    library is reported later by Dylint with exit `1`.
- 2026-11-24, revision 16: the review rounds after revision 15 are
  dispositioned: review `6028550043` (seven inline findings and the Domain
  Architecture row), review `6042229452` (two inline findings and one
  outside-diff finding), and the CodeScene advisories the new tests raised.
  - Library-path discovery now mirrors Dylint's own parser rather than the
    installer's staged-name parser: Dylint splits at the first `@` without
    rejecting an empty toolchain, so `libwhitaker_suite@.so` is a second
    library to its selection. A path observed more than once is loadable only
    when every observation called it a regular file, an `ENOTDIR` entry is a
    defect in the list rather than a host fault, listing stops at the first
    entry the user must fix, and a list that is not valid Unicode is refused
    because Dylint discards such a value in full.
  - The `check` port no longer names an installer type.
    `CheckPrepared::Bootstrap` carries the `LibraryRoute` and an `ApprovalId`;
    the concrete `PreparedRun` lives in `src/adapters/check/approval.rs`, and
    `src/adapters/check/fault.rs` translates `BootstrapError` into the
    port-owned `BootstrapFault`. `check_run` consumes the held run only when
    the approval number matches, so a value from an earlier preflight cannot
    run against a later one's installation.
  - `run_check_command` records its decision inside the `whitaker.check`
    span, so the event carries the invocation's span context.
  - The CodeScene advisories in the new tests are cleared by `rstest` case
    tables and a `Copy` expectation struct; those files score 10.00.
- 2026-11-25, revision 17: the review rounds on `a543ef82`, `d1af52bc` and
  `9e41a703` are dispositioned, and `main` is merged twice into the branch
  after the base advanced 16 commits and left the merge ref DIRTY, which
  stopped the PR's `pull_request` workflows entirely.
  - Direct approvals now carry an `ApprovalId` too, so the supersession rule
    holds on both routes rather than the bootstrap one only.
    `ApprovedRuns::consume` compares the number before taking anything, so a
    stale value is refused without consuming the current approval.
  - `observe_repair` takes a `Clock` and production passes `MonotonicClock`;
    `repair_production` takes a `RepairStreams` value instead of opening
    `std::io::stderr()`; the telemetry tests moved beside the module they
    cover; the command layer imports `Decision`, the `observe_*` entry points
    and `RepairableReason` through `ports`, which breaks the
    `adapters::check` ↔ `cli::check` cycle.
  - The merge keeps main's `candidates`/`explicit_tests` layout and its
    injected-value signature for `active_library_path`, and routes
    `classify_candidate` through the shared `classify_entry` seam. Main's
    new `bootstrap_install` fixture gained the `suite_version` field this
    branch added.
  - **Deferred with reasons, not dropped.** Three asks need the wider repair
    split rather than a local edit, and are recorded here so a follow-up can
    pick them up: moving the recorded-pin decision out of
    `src/adapters/check/repair.rs` into policy, expressing the `NotInstalled`
    fallback as a domain decision over typed readiness answers, and replacing
    the port's `io::Error` sources and `String` paths with typed values. Each
    changes the shape of the readiness answer itself; none changes behaviour,
    and each would touch every constructor and test case on the boundary.
  - `&mut self` on `approve_readiness` was implemented and reverted: the
    `Services` bundle holds the check port beside three shared ports, so a
    mutable accessor cannot reborrow `self` for the others. The rule is
    enforced by `ApprovalId` and stated in the port's documentation instead.
