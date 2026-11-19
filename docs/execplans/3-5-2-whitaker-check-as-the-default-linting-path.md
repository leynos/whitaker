# Implement `whitaker check` as the default linting path

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: COMPLETE (2026-11-18). The user accepted the plan, including decisions
D-1 and D-2, on 2026-11-17, and then instructed implementation to begin.
`EP-M0` to `EP-M3` are complete, and roadmap item 3.5.2 is marked done in
`docs/roadmap.md`. Every gate `EP-M3` names has been run on the final tree and
its result is recorded under `EP-M3`.

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
  - **Telemetry is emitted at the command boundary.** `whitaker.check.repairs`
    with an `outcome` label is recorded by `telemetry::record_repair` after the
    port call, and a test asserts a non-zero counter through
    `run_check_command` under a `DebuggingRecorder`.
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
    verified and 13 expected counterexamples observed, which is the full
    harness set. The first run on the final tree was red: see the 2026-11-19
    progress entry recording the assertion that had to change.
  - `make verus`: exit `0` in 6s over nine proof files, 68 verified, 0 errors.
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
- `src/adapters/mod.rs` holds `SystemServices`.
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

The method is Kani over the real generic `run_check_flow<P: CheckPorts>`. The
harness's `P` returns `kani::any()` choices. The domain is finite:

- 3 preflight classes per call, with 6 repairable reasons and 2 host faults;
- 2 repair results;
- 3 run results, with a symbolic `i32` code;
- 2 policies.

The exploration is therefore exhaustive. This is a complete check of the real
code, not a sample.

Verus is not used for the flow. Its state space is finite and small, so Kani's
exhaustive check of the real code gives a stronger guarantee than a Verus proof
of a model would. A Verus model would also add a gap between model and code for
no gain in coverage. This is a deliberate exception to proving business logic
in Verus, recorded so that reviewers can challenge it.

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

Non-vacuity:

- Reachability harnesses must each produce a counterexample for `Linted`
  after a repair, `Linted` after a run-time provisioning failure and its repair,
  `NotConverged`, `Refused` and `HostFailed`.
- Negative control: let the second `decide` use the caller's policy, and make
  its `Step::Repair` arm call `repair` again. Either change alone leaves the
  repair count at one: with the forbidden policy that arm is unreachable, and
  with the caller's policy it still returns `NotConverged`. `F3` must fail with
  a counterexample whose trace shows two repair calls.
- Negative control: classify `Io` as repairable. `F5` must fail.

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
- **Method.** An `rstest` truth table over the four combinations of
  `no_install` and `user_managed_path`, plus the BDD scenario. These are two
  Booleans, so a truth table is the proportionate tool.
- **Non-vacuity.** Each row asserts whether the fake repair port was called.
  Negative control: drop the `user_managed_path` term. The "user path is never
  repaired" row must fail.

### `CHK-INV-EXIT`: exit composition (new, exhaustive Kani)

- **Obligation**, stated over `check_exit_code(&CheckOutcome) -> i32`:
  - `Linted(code)` exits `code`, for every `i32`.
  - Every other outcome exits a value in `{3, 4, 5}`.
  - `{3, 4, 5}` is disjoint from `{0, 1}`.
  - `Refused`, `RepairFailed` and `NotConverged` exit `4`.
  - `HostFailed`, `RunFailed` and `OutputFailed` exit `5`.
  - `ConfigurationFailed` exits `3`. It is unreachable in phase one but is
    kept so that ADR 0017 is complete.
- **Method.** Kani with a symbolic `i32`, an `rstest` partition, and
  end-to-end tests against the real binary. The end-to-end tests prove that
  `main` does not discard the mapping.
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

- Verus proves two facts about models: routing is total and disjoint for
  argument vectors of any length, and argument assembly recovers the user's
  Cargo arguments for any length.
- Kani shows that the real `classify`, `run_check_flow`, `decide` and
  `check_exit_code` satisfy their invariants exhaustively over their finite
  domains.
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
