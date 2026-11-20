# 14.1.1 Implement the backend-neutral rewrite spike contract and shared fixtures

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (design review applied; awaiting user approval)

Branch: `14-1-1-spike-contract-for-borrow-only-clone-removal`

Roadmap item: `14.1.1` in `docs/roadmap.md`.

## Purpose / big picture

Whitaker plans to turn some ownership and borrowing lint findings into
compiler-validated source rewrites. A Dylint rule will describe a candidate
rewrite as a `RewriteIntent` with semantic anchors. A materializer will turn
that intent into precise byte-range text edits. A checker will then compile the
rewritten code twice: once under the ordinary borrow checker, which is based on
non-lexical lifetimes (NLL), and once under Polonius, the next-generation
borrow checker, which accepts some sound programs that NLL rejects.

Architectural decision record (ADR) 0007
(`docs/adrs/adr-0007-rust-rewrite-materialization-substrate.md`) has not yet
chosen the library that will power the materializer. Instead, it authorizes a
comparative spike. The same four rewrites are implemented twice, once with
ast-grep and once with rust-analyzer's `ra_ap_syntax::SyntaxEditor`. Both run
through one shared harness and fixture corpus, and the evidence picks the
winner.

This item builds the part of the spike that does not depend on either library:

- the backend-neutral contract both prototypes must implement: the
  `SpikeBackend` trait, the `SpikeIntent` and `SemanticCapture` input model, the
  `TextEditPlan` output model, and typed errors with stable diagnostic keys;
- a shared capture validator that every backend calls before it touches
  syntax;
- an edit-plan kernel that sorts, validates, and applies edits, with
  machine-checked proofs of its two key claims;
- the harness that owns everything ADR 0007 assigns to it: fixture snapshot
  integrity, edit sorting, overlap detection, edit application, unified diffs,
  fresh backend processes, compiler invocation, and the JSON report;
- four checked-in primary fixtures, each a small Cargo package with a
  hand-authored capture manifest, the exact expected edit ranges, and the
  expected rewritten source; and
- test-double backends that prove the harness rejects every failure the real
  backends will later be judged on.

Items 14.1.2 and 14.1.3 then add the two real backends, the adversarial corpus,
and the measurements. Nothing in this item adds ast-grep or `ra_ap_syntax` to
any dependency graph.

After this change a maintainer can run, from the repository root:

```bash
make spike-rewrite-fixtures
```

and see that the shared corpus is internally consistent and has the compiler
outcomes ADR 0007 requires. The command prints one line per fixture and exits
zero only when every expectation holds. Expected output, abbreviated:

```plaintext
toolchain: rustc commit 57d06900f (matches every fixture manifest)
probe: control accepted by both profiles; canary nll=rejected{E0499} polonius-next=accepted
own001-clone-only-borrow   before nll=accepted polonius-next=accepted | after nll=accepted polonius-next=accepted
own002-owned-param         before nll=accepted polonius-next=accepted | after nll=accepted polonius-next=accepted
bor001-conditional-lookup  before nll=accepted polonius-next=accepted | after nll=rejected{E0499} polonius-next=accepted
bor002-remove-reinsert     before nll=accepted polonius-next=accepted | after nll=accepted polonius-next=accepted
4 fixtures consistent: declared edits reproduce every expected output byte for byte; no new warnings
```

A second command runs the full ADR 0007 validation pipeline against a named
backend. In this item only test doubles are registered:

```bash
target/rewrite-spike/debug/rewrite-spike compare --backend double-replay --report target/rewrite-spike/report.json
```

It writes a JSON report in which every primary fixture passes every gate.
Running it with `--backend double-hash-order` produces a report in which
`own001-clone-only-borrow` fails the determinism gate.

No released Whitaker behaviour changes. The spike is a standalone Cargo
workspace that the root workspace does not include, and no shipped crate
depends on it.

## Constraints

- The spike lives under `spikes/rewrite-materialization/` as its own Cargo
  workspace with its own committed `Cargo.lock`. The root `Cargo.toml`
  `members` list must not include it, and no root workspace package may depend
  on it (ADR 0007 §Repository placement).
- No ast-grep crate and no `ra_ap_*` crate may appear in the spike's
  dependency graph in this item.
- Package dependencies must follow this direction:
  `spike_cli -> {spike_harness, spike_test_backends} -> spike_contract`, and
  `spike_harness -> spike_plan -> spike_contract`. No backend package, and not
  `spike_test_backends`, may depend on `spike_plan` or `spike_harness`.
  `spike_contract` depends on no spike package and performs no filesystem or
  process I/O.
- No contract type may contain a backend-specific node type (ADR 0007
  §Technical requirements).
- The harness must contain no fixture-specific code. All fixture knowledge
  lives in fixture manifests and fixture files (ADR 0007 §Mandatory gates).
- The harness must not preflight capture digests or capture ranges in the
  validation pipeline. Stale and invalid captures are rejected by the backend
  and observed only through the backend's result (ADR 0007 §Shared spike
  interface). Pipeline step 1 reads fixture file bytes and file digests only,
  never captures.
- The harness passes each backend only the source files the intent declares
  (ADR 0007 §Shared spike interface).
- Fixture files must be stored and checked out byte-exact. Git end-of-line
  conversion is disabled for `spikes/rewrite-materialization/fixtures/**` by a
  `.gitattributes` line placed after the repository-wide `*.rs text eol=lf`
  rule.
- Fixture `after/` trees are hand-authored. No tool may generate or "bless"
  them from the harness's own output.
- Contract, kernel, and harness output must be deterministic. No `HashMap`,
  `HashSet`, random state, wall-clock time, process identifier, or absolute
  path may appear in any serialized plan, outcome, diagnostic, or report.
- Repository conventions from `AGENTS.md` apply to all spike code: files of at
  most 400 lines, a `//!` module comment on every module, `///` comments on
  every function, no `unwrap` or `expect` outside tests, no `unsafe`,
  item-scoped `#[expect(..., reason = "...")]` only as a last resort, assertion
  messages on every `assert!`, caret version requirements, en-GB-oxendict
  spelling, and `cap_std`/`camino` for filesystem access.
- Production spike code reads environment variables only in the
  `rewrite-spike` binary's `main`. Child-process environments are set with
  `Command::env`, `env_remove`, and `env_clear`, which affect only the child.
- Compiler admission must not be bypassed. Nested Cargo runs keep the caller's
  `RUSTC_WRAPPER`, use the repository's pinned toolchain, and build under the
  root `target/rewrite-spike/` directory, never under `/tmp`.
- `docs/users-guide.md` does not change, because nothing user-visible ships.
- Completed ExecPlans under `docs/execplans/` must not be edited.
- Implementation begins only after the user explicitly approves this plan.

## Tolerances (exception triggers)

Every count below excludes this ExecPlan file. The comparison base is the merge
base with `origin/main` (`40a85aa5` when this plan was drafted). Counts are net
lines (additions minus deletions), measured with:

```bash
git diff --numstat "$(git merge-base origin/main HEAD)" -- \
  ':!docs/execplans/14-1-1-spike-contract-for-borrow-only-clone-removal.md'
```

Each file is assigned to exactly one category:

- Code: Rust source outside `fixtures/` and `tests/data/`, the Verus proof,
  the BDD `*.feature` files, Python under `tests/workflow_contracts/`, and the
  changed hunks of `Makefile` and `scripts/*.sh`.
- Fixture data: everything under `spikes/rewrite-materialization/fixtures/`
  and any `tests/data/` directory in the spike (Rust sources, fixture
  `Cargo.toml` files, and manifests), plus insta snapshot files. These are
  tracked separately because their size follows from the ADR's fixture
  requirements rather than from implementation effort.
- Configuration: the spike's `Cargo.toml` files, `clippy.toml`,
  `.config/nextest.toml`, `.gitattributes`, and `typos.local.toml`. The spike's
  `Cargo.lock` is generated and excluded from the count, but it must be
  committed.
- Documentation: Markdown outside `docs/execplans/`.

Budget survey. No earlier plan creates a standalone workspace. The nearest
analogues are `docs/execplans/7-1-1-whitaker-sarif-crate.md`, which added one
crate under a budget of 20 files and 2,000 net lines and finished at the file
cap, and
`docs/execplans/2-4-1-string-continuation-style-post-expansion-early-lint.md`,
which added two crates under a budget of 30 files and 1,800 net lines and
finished at 51 paths and 3,507 lines. The owner re-authorized 2-4-1; the
overrun came from approved milestone growth plus work already on the branch,
not from missed scope. `docs/execplans/6-1-1-shared-lcom4-helper.md` finished
exactly at its 400-line cap. No earlier plan kept per-category budgets.

The lesson is that new-crate plans land at or over a single combined cap. This
plan therefore sets per-category budgets from an explicit breakdown, and
milestone `EP-M4` is split in two so that no milestone carries more than about
1,500 lines. The design review argued for a code budget of 4,500, achieved by
deferring the Verus proof and the report. This plan keeps both. The user asked
for Verus proofs of introduced lemmas, and the report is a harness duty under
ADR 0007. Removing `--explain` and keeping the report minimal recovers part of
the difference.

The code estimate, by package:

- `spike_contract`: 700 lines of production code and 650 of tests.
- `spike_plan`: 300 production and 550 tests, including the bounded
  exhaustive test.
- `spike_harness`: 1,400 production and 800 tests.
- `spike_test_backends`: 400.
- `spike_cli`: 300 production, plus 450 for the BDD steps and end-to-end
  tests.
- Verus model: 300.
- Makefile, script, and lane-contract wiring: 100.

That totals about 6,000 lines, so the code budget below includes no
contingency, and that is deliberate. If the work goes over, the reason should
be reported, not absorbed.

- Files: if more than 120 non-ExecPlan files change, stop and escalate.
- Code: if code exceeds 6,000 net lines, stop and escalate.
- Fixture data: if fixture data exceeds 1,200 net lines, stop and escalate.
- Configuration: if configuration exceeds 450 net lines, stop and escalate.
  This includes about 90 lines of root lint tables copied into the spike
  manifest.
- Documentation: if documentation exceeds 600 net lines, stop and escalate.
- Interface: no root-workspace public API may change. Needing one is an
  escalation.
- Dependencies: only the dependencies listed in `Interfaces and dependencies`
  are authorized. Any other crate, any ast-grep or `ra_ap_*` crate, or any new
  root-workspace dependency is an escalation.
- Architecture: if the contract cannot express one of the four primary
  fixtures without a backend-specific type, or the harness needs
  fixture-specific code, stop and escalate. Either would falsify ADR 0007's
  shared-interface assumption.
- Toolchain: if the pinned nightly stops accepting `-Zpolonius=next`, or if
  `bor001`'s `after/` stops being rejected by NLL and accepted by Polonius,
  stop and escalate. Do not move the toolchain pin in this item.
- CI: if wiring the spike requires any change to
  `tests/workflow_contracts/` beyond adding the spike command patterns and the
  `SPIKE_REWRITE_TEST=0` override assertions, stop and escalate.
- Verus: if a Verus obligation is not discharged after one working day, or a
  proof run takes longer than 10 minutes, stop and escalate. Do not weaken a
  theorem, and do not add `assume`, `admit`, or `external_body` to make it pass.
- Iterations: if a gate still fails after three fix attempts for the same
  cause, stop and escalate.
- Time: if any one milestone takes more than two working days, stop and
  escalate.
- Ambiguity: if ADR 0007 and RFCs 0007 to 0009 conflict in a way that
  `Decision log` does not already resolve, stop and present the options.

Each budget is enforced on its own. Spare capacity in one category does not
offset an overrun in another.

## Risks

- Risk: the standalone workspace inherits root configuration in surprising
  ways. Rustfmt, Cargo configuration, and `rust-toolchain.toml` are found by
  searching parent directories. Clippy's `clippy.toml` and nextest's
  configuration may or may not be. Severity: medium. Likelihood: medium.
  Mitigation: the `EP-M0` probe records exactly what the spike inherits before
  any code is written. The spike carries its own `clippy.toml` and
  `.config/nextest.toml` regardless.

- Risk: nested Cargo runs inherit the environment that `make test` and
  `make coverage` set up. That environment includes `RUSTUP_TOOLCHAIN`, `CARGO`,
  `CARGO_TARGET_DIR`, three kinds of `RUSTFLAGS`, `LLVM_PROFILE_FILE`, and
  profile overrides. Any of these could change which compiler runs, or where it
  writes, and so falsify a fixture outcome. Severity: high. Likelihood: high
  without mitigation. Mitigation: the profile runner scrubs a fixed list of
  variables (see `EP-M3`). Before any profile run, it checks that `rustc -vV`
  in the scrubbed environment reports the manifest's commit hash. A compile
  counts as accepted only if Cargo reports both a `compiler-artifact` message
  for the fixture's library and a successful finish.

- Risk: when every compile shares one target directory, Cargo's
  timestamp-based freshness check can reuse an earlier result. Severity: high.
  Likelihood: medium. Mitigation: each compile gets a fresh target directory
  inside a per-invocation temporary directory under `target/rewrite-spike/`.
  The fixtures have no dependencies, so a fresh directory costs almost nothing.

- Risk: `make test` is reached through `make coverage` (Linux CI),
  `make coverage-build` (compile-only warm-up), and `make trybuild-contracts`.
  `coverage-build` swaps `CARGO` for a stand-in that compiles and runs nothing,
  so a spike run there would misbehave. Severity: high. Likelihood: certain
  without mitigation. Mitigation: `test` runs the spike only when
  `SPIKE_REWRITE_TEST` is 1. `coverage-build` and `trybuild-contracts` set it
  to 0, and a workflow-contract test asserts both overrides. `coverage` leaves
  it at 1, so Linux CI runs the spike once, uninstrumented, in the
  `coverage-check` lane. That matches the lane-deduplication policy that
  coverage is Linux's single test execution.

- Risk: the Windows lane (`windows-compat`, a required check) runs
  `make test NEXTEST_PROFILE=ci`. A nextest profile the spike does not define
  is a hard error, and process-heavy spike tests are slow and fragile on
  Windows. Severity: medium. Likelihood: high without mitigation. Mitigation:
  the spike's nextest configuration defines both `default` and `ci` profiles.
  On Windows, `spike-rewrite-test` runs only the pure packages
  (`spike_contract` and `spike_plan`), whose tests also check `/`-separated
  path handling. It prints a message naming the skipped packages and the reason
  (`D10`).

- Risk: nested builds wait in the shared compile-admission queue long enough
  to time out. This repository has recorded nested-Cargo tests needing 30, 75,
  and 120 minutes (`docs/developers-guide.md`, "Test timeouts: four tiers").
  Severity: medium. Likelihood: medium. Mitigation: a compile outcome has three
  states. A deadline hit, a spawn failure, or a missing artefact is
  `Inconclusive`, which fails the run but is never reported as a rejection. All
  real-compiler tests live in one test binary, in a nextest group with
  `max-threads = 1`, a 75-minute allowance, and no retries. Within one
  invocation, outcomes are cached by profile and package content digest, so
  each fixture side is compiled once per profile.

- Risk: the Polonius behaviour that `bor001` depends on changes when the
  toolchain pin moves. Severity: medium. Likelihood: low for this item.
  Mitigation: the capability probe runs before every profile run and reports
  `unavailable` rather than a misleading outcome. The fixture README records
  that the expectation must be re-checked on every toolchain bump (ADR 0007
  §Known risks).

- Risk: hand-authored byte offsets are easy to get wrong. Severity: medium.
  Likelihood: high. Mitigation: every capture and expected edit also records
  its literal text. The corpus check verifies range, digest, and text together.
  When they disagree, it prints the bytes actually found at the declared range.
  It also verifies that applying the declared edits to `before/` reproduces
  `after/` exactly.

- Risk: the shared interface favours one backend by accident (ADR 0007
  §Known risks). Severity: medium. Likelihood: medium. Mitigation: the contract
  carries only byte ranges, digests, opaque identities, and an `ExpectedSyntax`
  vocabulary defined in Rust Reference grammar terms with explicit extent
  rules. It is not defined in terms of either library's node kinds. The
  edit-granularity rule in `D4` is chosen so that it does not reward whole-node
  replacement over structured editing. Review checks every contract type for
  library-shaped assumptions.

- Risk: a backend is eliminated by infrastructure noise rather than its own
  behaviour. For example, one of its 100 processes is starved during an
  admission storm. Severity: high for 14.1.3. Likelihood: low. Mitigation:
  spawn failures, deadline hits, and output-cap breaches are reported as
  `Inconclusive` with their cause, never as plan divergence. The report records
  every inconclusive run.

## Progress

- [x] (2026-11-16 12:10Z) Branch renamed and session titled.
- [x] (2026-11-16 12:20Z) Reconnaissance: ADR 0007; RFCs 0007, 0008, and 0009;
  repository gates and CI; past plan budgets; design and testing guides.
- [x] (2026-11-16 12:30Z) Planning probe: the pinned nightly's NLL and Polonius
  behaviour recorded.
- [x] (2026-11-16 13:30Z) Initial draft written and committed.
- [x] (2026-11-16 14:40Z) Design review by a four-member expert panel
  (structure and contracts; failure modes and verification; cost and
  maintainability; alternatives and fixture fidelity).
- [x] (2026-11-16 14:45Z) Re-probe of the revised `bor001` and `bor002` shapes
  on the pinned nightly.
- [x] (2026-11-16 15:30Z) Review revisions applied (see `Revision note`).
- [ ] Plan approved by the user.
- [ ] `EP-M0` workspace and inheritance probe.
- [ ] `EP-M1` spike workspace, gate wiring, contract model, and capture
  validation.
- [ ] `EP-M2` edit-plan kernel and Verus proofs.
- [ ] `EP-M3` primary fixtures, corpus check, and checker profiles.
- [ ] `EP-M4a` process protocol, test backends, and determinism runner.
- [ ] `EP-M4b` validation pipeline, report, and conformance suite.
- [ ] `EP-M5` documentation, reconciliation, and roadmap closure.

## Surprises & discoveries

- Observation: on the pinned toolchain `nightly-2026-05-28`
  (`rustc 1.98.0-nightly (57d06900f 2026-05-27)`), the default borrow checker
  is NLL, `-Zpolonius=next` selects Polonius Alpha, and `-Zpolonius=off` is
  rejected as an invalid value. Evidence: the planning probe in
  `Artefacts and notes`. Impact: on this toolchain the NLL profile passes no
  `-Zpolonius` flag at all. Current nightlies default to Polonius and need
  `-Zpolonius=off` for NLL, so the profile runner chooses flags by probing
  rather than hard-coding them (`D5`).

- Observation: the repository's `.gitattributes` forces `*.rs text eol=lf`,
  and Git applies the last matching line. Evidence: `.gitattributes` lines 1 to
  9. Impact: without an override placed after that line, a CRLF fixture (which
  14.1.2 needs) would be normalized at checkout, and its digests would change
  silently.

- Observation: `make coverage`, `make coverage-build`, and
  `make trybuild-contracts` all reach the `test` recipe through `$(MAKE) test`
  (`Makefile` lines 399, 419, and 433). Linux CI therefore does run `make test`
  indirectly, through the `coverage-check` lane. `linux-full` must run no tests
  (`tests/workflow_contracts/lane_deduplication_contract_test.py`,
  `test_linux_full_executes_no_tests`). Impact: the first draft's plan to add a
  `linux-full` test step was wrong and has been replaced (`D10`).

- Observation: RFC 0007 §6.2 requires plan edits to arrive sorted. ADR 0007
  makes sorting a harness duty. No RFC addresses zero-width insertions, edits
  that share a start offset, or edits whose replacement equals the original.
  Evidence: RFC 0007 lines 285 to 296 and 1044 to 1049; ADR 0007 lines 384 to
  394. Impact: `D4` settles these semantics for the spike, and `EP-M5` records
  the difference from RFC 0007 as an explicit deviation.

- Observation: RFC 0008 §remove_then_reinsert suppresses the lint when an
  early return or `?` lies between the removal and the reinsertion, because
  then the reinsertion can be skipped. Evidence: RFC 0008 line 483. Impact: the
  first draft's fixture 4 put a `?` in that region, which is a shape the
  detector would never emit. The revised fixture keeps every `?` outside the
  region (`D13`).

## Decision log

- D1. Decision: scope. This item delivers the contract, the shared capture
  validator, the edit-plan kernel and its proofs, the harness pipeline, the
  four primary fixtures, test-only rejection fixtures, a minimal JSON report,
  and test-double backends. It does not deliver either real backend, the
  adversarial corpus, timing or footprint measurements, the spike report
  document, or the ADR outcome. The contract and manifest schema do define
  every typed error, expectation kind, and harness check the adversarial corpus
  will need, so that 14.1.2 adds fixtures and backends without changing the
  contract. Rationale: the roadmap splits the spike into contract and fixtures
  (14.1.1), backends and adversarial fixtures (14.1.2), and evidence and
  selection (14.1.3). A harness built before either real backend exists can
  only be proved against doubles, so the doubles are part of this item's
  evidence, not throwaway scaffolding. Date/Author: 2026-11-16, planning agent;
  revised after design review.

- D2. Decision: the spike workspace has five packages under
  `spikes/rewrite-materialization/crates/`:
  - `spike_contract`: the domain types, the `SpikeBackend` port, the
    capture validator, and the errors;
  - `spike_plan`: the edit-plan kernel;
  - `spike_harness`: a library-only harness, with the backend injected;
  - `spike_test_backends`: the test doubles, which depend on `spike_contract`
    only; and
  - `spike_cli`: the `rewrite-spike` binary, the backend registry, and the
    end-to-end and BDD tests.

  14.1.2 adds one package per real backend, each depending only on
  `spike_contract`, plus one registry line in `spike_cli`. A dependency-rule
  test in `spike_cli` parses every spike package manifest and enforces the
  graph in `Constraints`.

  Rationale: ADR 0007 §Migration plan requires each backend to be implemented
  without importing the other library, and separate packages make Cargo enforce
  that. Putting the binary in its own composition-root package removes the
  cycle the first draft had: the harness binary depended on the backends, while
  the backends' tests depended on the harness. It also gives the conformance
  tests access to `CARGO_BIN_EXE_rewrite-spike`, which Cargo sets only for
  tests in the binary's own package. Keeping the kernel out of `spike_contract`
  stops a backend from sorting or validating its own plan, which would hide the
  emission-order nondeterminism that pipeline step 5 exists to detect.

  ADR 0007's file tree shows a single `src/` directory, but it is illustrative
  ("should live"). This plan keeps that tree's top-level items: `Cargo.toml`,
  `Cargo.lock`, `README.md`, `fixtures/`, and the harness and backend roles.
  The rejected alternative is a single crate with Cargo features per backend.
  Features are unified across a workspace build, so they cannot enforce the
  separation. Date/Author: 2026-11-16, planning agent; revised after design
  review.

- D3. Decision: capture validation is a contract function,
  `spike_contract::prepare_intent`, that every backend calls first. It returns a
  `PreparedIntent` whose `ValidatedCapture` values carry the checked snippet
  text. The harness never calls it in the pipeline. The typestate is a
  convenience, not an enforcement mechanism: `materialize` still receives the
  raw source map, so a backend could skip the call. Enforcement comes from the
  rejection fixtures and the `double-stale-blind` control, which fail any
  backend that does not reject a stale capture. The ADR text says "each backend
  checks", and a shared validator makes capture-fault behaviour identical
  across backends by construction. 14.1.3's report must say so rather than
  present identical capture-fault results as independent evidence. Rationale:
  ADR 0007 requires each materializer to reject stale and invalid captures, and
  requires that the harness not preflight them. A shared validator gives
  byte-identical rejection behaviour across backends, and keeps validation code
  out of the backend line counts that 14.1.3 compares. The rejected alternative
  is for each backend to write its own checks. That duplicates logic the ADR
  wants to behave identically, and charges one backend for code the other might
  express more tersely. Date/Author: 2026-11-16, planning agent; revised after
  design review.

- D4. Decision: edit-plan semantics for the spike.

  The rules:
  - A `ByteRange` is half-open, `[start, end)`, in bytes of the original file,
    and is serialized as two `u64` values.
  - The harness, not the backend, canonicalizes a plan. It sorts files by
    path, and sorts each file's edits by `(start, end, replacement bytes)`.
  - A canonical file edit list is well formed when:
    - every range lies within the file;
    - both offsets of every range are UTF-8 character boundaries;
    - no edit is a no-op: a no-op is an edit whose replacement equals the
      bytes it replaces, which includes an empty range with an empty
      replacement;
    - starts are strictly increasing; and
    - each edit's `end` is at most the next edit's `start`.
  - Two edits that share a start offset conflict, even when one is a
    zero-width insertion, because otherwise their output order would depend on
    the order they were emitted in. Edits that touch end to start are allowed.
  - A plan names each file at most once, has no file entry with an empty edit
    list, and edits only files declared in the intent.
  - Canonicalization never merges adjacent edits.
  - Granularity rule: one edit per replaced or deleted syntactic unit, as
    `ExpectedSyntax` defines those units. A deleted statement owns its leading
    indentation and its trailing newline. Fixture manifests are written to
    this rule, and the exact range-set gate compares against them.
  - A canonical plan records each file's digest, and application takes the
    source map and refuses a file whose digest differs.
  - Any `PlanViolation` counts against the backend that produced the plan.

  Rationale: the Language Server Protocol (LSP) defines edit ranges against the
  original document, forbids overlap, and orders same-position insertions by
  their position in the array.[^lsp] Ordering by array position conflicts with
  the harness owning sorting, so the spike rejects that case instead. RFC 0007
  §6.2 already requires non-overlap, bounds, UTF-8 boundaries, and unique
  paths. This decision deviates from RFC 0007 in three recorded ways:
  - unsorted input is normalized rather than rejected, because ADR 0007 makes
    sorting a harness duty;
  - the same-start and no-op rules are added; and
  - `FileEdits` carries no base digest, because the intent's `FileIntent`
    already does.

  `EP-M5` records these deviations in ADR 0007 and adds a forward note to RFC
  0007 §6.2, so that 14.2.1 finds them. Date/Author: 2026-11-16, planning
  agent; revised after design review.

- D5. Decision: checker profiles. The harness supports two profiles, `nll`
  and `polonius-next`, using RFC 0007's `BorrowCheckerProfile` wire names.

  The procedure:
  - Before any fixture run, the harness checks that `rustc -vV`, run in the
    scrubbed environment, reports the commit hash recorded in every fixture
    manifest. A mismatch makes both profiles `unavailable`.
  - It then compiles a control package, which every candidate flag set must
    accept, and a canary package, a minimal conditional reborrow.
  - The `nll` profile uses the first of "no flag" and `-Zpolonius=off` for
    which the control is accepted and the canary is rejected with exactly
    `{E0499}`.
  - The `polonius-next` profile uses `-Zpolonius=next`, which must accept both
    packages.
  - If no flag set qualifies, that profile is `unavailable`, and every
    expectation under it fails as `Inconclusive`.

  On the pinned nightly this selects "no flag" for `nll`. Rationale: the
  planning probe showed that this nightly rejects `-Zpolonius=off`, while
  current nightlies default to Polonius and need it for NLL. RFC 0007
  §Capability probing already requires a probe, because the flag appearing in
  `rustc -Z help` does not prove that it works. The control package stops a
  canary that is rejected for an unrelated reason, such as a leaked
  `-D warnings` or a wrapper failure, from selecting the wrong flag.
  Date/Author: 2026-11-16, planning agent; revised after design review.

- D6. Decision: fixture representation.

  Layout:
  - Each fixture is a directory `fixtures/<fixture-id>/` holding a
    `manifest.toml`, a `before/` Cargo package, and an `after/` Cargo package.
  - Each package's `Cargo.toml` declares an empty `[workspace]` table and no
    dependencies, and uses edition 2024.
  - ADR 0007's top-level `expected/` directory becomes each fixture's `after/`
    directory, so each fixture's input and expected output sit together.

  The manifest records:
  - a `line_endings` value of `lf`, `crlf`, or `mixed`, which the check
    enforces;
  - the digest of every source file;
  - the intent;
  - the captures, each with its literal text;
  - an `expect` table, which is either `kind = "success"` with the exact
    edits (each with its literal original text) and the compiler outcomes,
    or `kind = "backend_rejection"` with the expected diagnostic key, capture
    name, and path;
  - the recipe metadata (`D8`); and
  - an optional `sources` key naming another fixture's `before/` directory, so
    an adversarial fixture can reuse a primary fixture's sources.

  String values in manifests are single-line basic strings with explicit `\n`
  and `\r\n` escapes. The loader rejects any manifest containing a multi-line
  string, because TOML parsers may normalize newlines inside them and trim the
  newline that follows the opening delimiter.

  Rationale: compiler validation needs real Cargo packages, and the
  repository's "group by feature" rule favours colocation. The literal text
  makes hand-authored offsets reviewable. The `expect` table lets 14.1.2 add
  rejection fixtures without changing the schema. Date/Author: 2026-11-16,
  planning agent; revised after design review.

- D7. Decision: synthetic semantic identity. Fixtures are hand-authored, so
  their `compiler_identity` and `node_identity` values cannot come from a real
  compiler run. The manifest records the pinned compiler's commit hash as
  `compiler_identity`. Each `node_identity` is a structured value: an item path
  for items, or an owner path plus a syntax fingerprint for expressions. The
  manifest marks every identity `synthetic = true`. Backends must treat
  identities as opaque and may only compare them for equality. Rationale: ADR
  0007 asks for hand-authored captures "equivalent to those a Dylint rule would
  emit", so that the spike measures materialization rather than discovery.
  Marking the values synthetic stops anyone mistaking them for compiler
  evidence. Date/Author: 2026-11-16, planning agent.

- D8. Decision: recipes and recipe metadata.

  Recipe identifiers:
  - Identifiers serialize as RFC 0009's stable dotted kinds with an integer
    version: `ownership.clone_only_used_by_borrow`,
    `ownership.owned_param_causes_clone`, and `borrow.lookup_then_relookup`.
  - The fourth recipe uses `borrow.remove_then_reinsert`, which RFC 0009 §8.3
    does not yet list.
  - An unrecognized kind deserializes to `RecipeKind::Unknown(String)`, so the
    intent stays inspectable. `prepare_intent` then rejects it with
    `UnsupportedRecipe`, following RFC 0009's rule for unknown kinds.

  Each recipe schema declares:
  - its capture roles, with a count for each;
  - the `ExpectedSyntax` each role must have; and
  - whether the recipe accepts a capture located at an editable macro call
    site. None of the four primary recipes does, following RFC 0009 §7.3.

  Recipe metadata, recorded in every manifest and carried unchanged into the
  report:
  - `confidence`: `mechanical`, `locally_reasoned`, or `review_required`;
  - `checker_requirement`: `nll` or `polonius-next`;
  - `may_change`: a list drawn from `evaluation_order`, `panic`, `drop_order`,
    `map_order`, and `allocation`; and
  - `signature_change`: `none` or `private`.

  Rationale: these names keep the spike aligned with the production intent
  model. The missing kind is recorded as a gap for RFC 0009 rather than
  invented silently. Date/Author: 2026-11-16, planning agent; revised after
  design review.

- D9. Decision: verification mix.
  - Verus proves three claims about the edit-plan kernel. The proofs work over
    an independent sequence model whose specification uses last-edit-first
    recursion, in the style of `verus/string_continuation_rewrite.rs`. The
    claims are: checking adjacent pairs implies pairwise disjointness, the
    production algorithm's forward-cursor shape equals the recursive
    specification, and an offset-map statement of untouched-byte
    preservation.
  - `proptest` checks the compiled kernel against oracles that share no code
    with it: a pairwise overlap check, and end-to-start application with
    `String::replace_range`.
  - A bounded exhaustive test enumerates every plan of up to three edits over
    a four-byte body.
  - `rstest` tables and a construction-based property cover capture
    validation.
  - `rstest-bdd` covers the observable harness behaviour.
  - Kani is not used.

  Two panel members recommended deferring the Verus proof to 14.2.1, where the
  production kernel will live. This plan keeps it, for two reasons. The `D4`
  semantics are introduced here, and the user asked for Verus proofs of
  introduced lemmas. The proof also transfers to 14.2.1 unchanged, because it
  models the semantics rather than the spike's types. 14.2.1 must revisit Kani
  for the compiled kernel when it lifts the code. See
  `Formal verification decision`. Date/Author: 2026-11-16, planning agent;
  revised after design review.

- D10. Decision: gate wiring.

  New Makefile targets drive the spike through `--manifest-path`:
  `spike-rewrite-fmt`, `spike-rewrite-check-fmt`, `spike-rewrite-lint`,
  `spike-rewrite-test`, `spike-rewrite-check` (all three checks), and
  `spike-rewrite-fixtures`. All of them build into `target/rewrite-spike/` and
  pass `$(CARGO_LOCKED)`.

  Wiring into the root targets:
  - `fmt`, `check-fmt`, and `lint` always call their spike counterparts.
  - `test` calls `spike-rewrite-test` when `SPIKE_REWRITE_TEST` is 1, which
    is the default. `coverage-build` and `trybuild-contracts` set it to 0.
    `coverage` leaves it at 1, so the spike runs uninstrumented inside the
    Linux coverage lane.
  - `fetch` also fetches the spike's dependencies.

  How `spike-rewrite-test` runs:
  - It calls `$(CARGO_NEXTEST) nextest run` directly, never `$(TEST_RUNNER)`.
  - It sets its own `CARGO_TARGET_DIR`.
  - It passes `--no-tests=fail`.
  - It passes `--profile` only when `NEXTEST_PROFILE` is set.
  - On Windows it runs only `spike_contract` and `spike_plan`, and prints the
    names of the skipped packages and the reason.

  No CI workflow changes. `tests/workflow_contracts/` gains:
  - the spike command patterns in `TEST_COMMANDS`; and
  - assertions that `coverage-build` and `trybuild-contracts` set
    `SPIKE_REWRITE_TEST=0`.

  The spike is excluded from the LCOV report and from the Whitaker Dylint run.

  Rationale: ADR 0007 says a repository target "may run the spike explicitly".
  Wiring it into the root gates goes further than that, and the choice is
  deliberate. The user requires `make check-fmt`, `make lint`, and `make test`
  to pass after every milestone, and a standalone workspace is otherwise
  invisible to every one of them. The rejected alternative was a path-filtered
  `spike.yml` workflow, with `make test` left untouched. It costs nothing in
  the main lanes, but local `make test` would then say nothing about the spike.
  Date/Author: 2026-11-16, planning agent; revised after design review.

- D11. Decision: determinism, process protocol, and resource bounds.

  Data structures:
  - Spike code uses `BTreeMap`, `BTreeSet`, and `Vec` only.
  - The spike's `clippy.toml` copies the root settings and adds
    `disallowed-types` for `std::collections::HashMap` and `HashSet`. The
    `double-hash-order` negative control carries the single item-scoped
    `#[expect(clippy::disallowed_types, reason = ...)]`.

  Process protocol:
  - A backend process writes its result to a file named by `--outcome`, never
    to standard output, so stray prints cannot corrupt it.
  - The result is a `MaterializeOutcome`. It carries an
    `outcome_schema_version` and is either `Planned { raw_plan }` or
    `Rejected { error, diagnostic }`, holding the serialized `SpikeError` and
    its `StableDiagnostic`.
  - The determinism gate compares the outcome file's bytes and the exit status
    across runs.

  Process runner bounds:
  - It reads standard output and standard error concurrently.
  - It caps the outcome file at 1 MiB and each stream at 64 KiB. Exceeding a
    cap is a typed failure, never a silent truncation.
  - It enforces a 60-second deadline by polling `try_wait`, then kills and
    reaps the child.
  - It sets `RUST_BACKTRACE=0` and removes `RUST_LOG` for the child.
  - Spawn failures, deadline hits, and cap breaches are `Inconclusive`.

  Determinism runs:
  - The pipeline always uses exactly 100 runs, and the report records the run
    count.
  - A `--runs` option exists for tests. A report with fewer than 100 runs
    marks the determinism gate `not_evidential`, so it can never make a
    backend eligible.

  Date/Author: 2026-11-16, planning agent; revised after design review.

- D12. Decision: documentation placement.
  - ADR 0007 is still `Proposed`, so its sections are amended in place rather
    than through a new ADR. §Repository placement records the `D2` and `D6`
    layout. §Shared spike interface gains a "Contract refinements (14.1.1)"
    subsection recording `D3` to `D8`, the RFC 0007 deviations from `D4`, the
    `ExpectedSyntax` definitions, and the stable diagnostic-key table.
    §Outstanding decisions records the missing `borrow.remove_then_reinsert`
    kind for RFC 0009, and notes that the NLL flag depends on the toolchain.
  - RFC 0007 §6.2 gains a short forward note pointing at those refinements.
  - `spikes/rewrite-materialization/README.md` is the spike's component
    architecture document.
  - `docs/developers-guide.md` gains a pointer of at most five lines, because
    that guide outlives the spike.
  - `docs/repository-layout.md` gains a `spikes/` entry.
  - `docs/users-guide.md` does not change.

  Date/Author: 2026-11-16, planning agent; revised after design review.

- D13. Decision: fixture shapes follow what the lints would emit and what
  RFC 0009 permits. The four shapes are given in `EP-M3`. They were revised
  after the design review found three problems in the first draft:
  - fixture 4 put a `?` between removal and reinsertion, which RFC 0008
    suppresses;
  - fixture 3 moved a branch comment outside its branch, and captured a
    default it never used; and
  - fixture 2 had no imports to preserve.

  Fixture 1 clones a `String`. RFC 0009 trusts identity clones only for
  primitive `Copy` scalars, shared references, and tuples or arrays of those,
  so fixture 1 is `review_required`, with `allocation` in `may_change`.
  Rationale: ADR 0007 asks for captures equivalent to those a Dylint rule would
  emit, so fixtures must not exercise shapes the detectors reject. Date/Author:
  2026-11-16, planning agent.

## Outcomes & retrospective

Not started. Record outcomes at each milestone boundary.

## Context and orientation

Inspected base revision: `40a85aa5` (`origin/main`, "Score desugared loops and
`?` as one authored construct (#623)").

### The rewrite pipeline and where this item sits

Three proposed RFCs describe the pipeline.

- RFC 0007 (`docs/rfcs/rfc-0007-compiler-validated-rewrite-checking.md`)
  defines the checker that replays a candidate under NLL and Polonius.
- RFC 0008 (`docs/rfcs/rfc-0008-borrow-workaround-lint-family.md`) defines
  the borrow-workaround lint family. Its codes `BOR001`
  (`lookup_then_relookup`) and `BOR002` (`remove_then_reinsert`) motivate
  fixtures 3 and 4.
- RFC 0009 (`docs/rfcs/rfc-0009-validated-ownership-and-borrow-rewriter.md`)
  defines the rewriter and its materializers, including
  `clone_only_used_by_borrow` and `owned_param_causes_clone`, which motivate
  fixtures 1 and 2.

All three are `Proposed`. ADR 0007 sits between them and the code: it decides
how edits are materialized, and its §Comparative spike is the normative
specification for this item. Read ADR 0007 §Comparative spike in full before
starting.

Vocabulary this plan aligns with:

- RFC 0007 §6.2 defines
  `TextEdit { start_byte: u64, end_byte: u64, replacement: String }` and
  `FileEditSet { path, base_sha256, edits }`. It requires edits that are
  sorted, non-overlapping, in bounds, and on UTF-8 boundaries, and applies them
  end to start.
- RFC 0009 §7.1 defines `SemanticAnchor` with `base_file_sha256`, a byte
  `span`, and `snippet_sha256`. §7.2 says never to guess when several matches
  remain. §7.3 rejects macro-expansion-only spans unless a recipe opts in to
  macro call sites.
- RFC 0007 names the checker profiles
  `BorrowCheckerProfile { Nll, PoloniusNext }`, written `nll` and
  `polonius-next` on the command line.
- RFC 0008 and RFC 0009 define
  `SemanticConfidence { Mechanical, LocallyReasoned, ReviewRequired }`.
- RFC 0009 §8.3 defines stable dotted recipe kinds with independent integer
  versions.

ADR 0007 refers to several types it never defines: `ByteRange`, `TextEditPlan`,
`SpikeError`, `RecipeId`, `FileIntent`, `CompilerIdentity`,
`SemanticNodeIdentity`, `ExpansionProvenance`, `Sha256Digest`,
`ExpectedSyntax`, and `WorkspaceRelativePath`. This item defines all of them
(see `Interfaces and dependencies`). The spike's names stay close to the RFCs'
names, but the spike types are not the production model. Roadmap item 14.2.1
defines the versioned production `RewriteIntent` and `TextEditPlan` in
`whitaker_rewrite_model`, informed by the spike.

### Terms used in this plan

- Capture: one `SemanticCapture`. It is a named, compiler-anchored byte range
  in one source file, with digests of the whole file and of the range's bytes.
- Capture name: a string of the form `<role>` or `<role>.<index>`, for
  example `use.0` and `use.1`. A recipe schema counts captures per role.
- Stale capture: a capture whose file digest or snippet digest no longer
  matches the supplied source.
- Materialize: turn an intent and its source files into a `TextEditPlan`
  without writing anything.
- Raw plan: the exact bytes of a backend process's outcome file, before the
  harness sorts or validates anything.
- Canonical plan: the harness's sorted and validated form of a raw plan.
- Checker profile: one borrow-checker configuration, `nll` or
  `polonius-next`.
- Compile outcome: `Accepted`, `Rejected { codes }`, or
  `Inconclusive { cause }`.
- Test double: a backend used only to test the harness. It is either well
  behaved or deliberately faulty. Every double's name starts with `double-`.
- Conformance suite: the pipeline run over every fixture, including the
  test-only rejection fixtures, for one named backend. 14.1.2 runs it against
  both real backends.

### Repository facts that shape the plan

The root workspace (`Cargo.toml` line 2) lists `cli`, `common`, `crates/*`,
`installer`, `suite`, and `vendor/dylint_testing`, and has no `exclude`. No
standalone Cargo workspace exists in the repository today. A package nested
under the root without its own `[workspace]` table is rejected by Cargo, as
`Makefile` lines 123 to 126 already note for scratch trees. `Makefile` line 275
anticipates "future committed standalone" manifests being fetched with
`cargo fetch --locked --manifest-path`.

The root lint policy lives in `[workspace.lints.clippy]`,
`[workspace.lints.rust]`, and `[workspace.lints.rustdoc]` (`Cargo.toml` lines
145 to 222). Among others, it denies `unwrap_used`, `expect_used`,
`indexing_slicing`, `string_slice`, `missing_docs`, `missing_const_for_fn`,
`must_use_candidate`, `shadow_reuse`, and `cognitive_complexity`, and it forbids
`unsafe_code`. A separate workspace inherits none of this, so the spike copies
it, and a parity test keeps the copy at least as strict.

Several root configuration files are found by searching parent directories, so
spike sources and fixture copies under `target/rewrite-spike/` should pick them
up:

- `.rustfmt.toml` enables unstable features (`imports_granularity`,
  `group_imports`, `wrap_comments`, and `fn_single_line`). `EP-M0` verifies
  that the spike picks it up.
- `.cargo/config.toml` sets `[unstable] no-embed-metadata = true`.
- `rust-toolchain.toml` pins `nightly-2026-05-28`, so the spike and every
  fixture copy use the same compiler. Rustup finds it by the same parent search.

`make test` exports `RUSTUP_TOOLCHAIN` and `CARGO` (`Makefile` lines 23 and
326), which is why the profile runner scrubs them.

Gate targets, with `Makefile` line numbers at `40a85aa5` (relocate them with
`grep -n '^<target>:' Makefile`):

- `fetch` (274);
- `test` (283), which also runs `test-root-lib` and `test-nextest-filter`;
- `coverage` (372), which calls `$(MAKE) test` with `CARGO_TARGET_DIR` and
  `TEST_RUNNER` overridden;
- `coverage-build` (405), which calls it with a compile-only `CARGO`
  stand-in;
- `trybuild-contracts` (423);
- `lint` (567), which runs `interrogate`, `skill-manifest-check`,
  `lint-clippy`, and `lint-whitaker`;
- `fmt` (582) and `check-fmt` (587);
- `markdownlint` (591), which depends on `spelling` (594);
- `nixie` (597); and
- `verus` (648).

`cargo fmt --all` formats only workspace members, so the root target never
reaches the spike. `mdtablefix --git` and `markdownlint` do scan Markdown under
`spikes/`, and `typos` scans tracked text files.

CI (`.github/workflows/ci.yml`):

- `linux-full` runs `make check-fmt` (line 492), `make lint` (589),
  `spelling`, `nixie`, Markdown lint, and `make test-workflow-contracts`. It
  must run no tests.
- `coverage-check` runs `make coverage`, which is Linux's single test
  execution.
- `windows-compat` runs `make test NEXTEST_PROFILE=ci` (line 818).
- `.github/workflows/proofs.yml` runs `make verus` and `make kani`. Its
  `paths` filter (lines 26 to 39) includes `verus/**` and `scripts/**`.

Verus proofs are standalone model files in `verus/`, one per property. Each is
listed by name in a group in `scripts/run-verus.sh` and run through
`make verus` or a named target. The pinned Verus release is in
`tools/verus/VERSION` (`0.2026.03.17.a96bad0`). The closest precedent is
`verus/string_continuation_rewrite.rs`, which proves that removing sorted,
disjoint byte ranges equals concatenating the surrounding fragments. Its
`ByteRange` model and last-range-first recursion are the template for this
item's specification. Kani harnesses live under `#[cfg(kani)]` inside
production crates.

The root `Cargo.lock` has these versions, which the spike should match:

- `rstest` 0.28.0, `rstest-bdd` 0.6.0, `googletest` 0.14.4,
  `pretty_assertions` 1.4.1, `insta` 1.49.0, and `proptest` 1.12.0;
- `sha2` 0.11.1, `serde_json` 1.0.154, `cap-std` 4.0.3, and `camino` 1.2.6;
- `similar` 2.7.0, which the root pulls in only transitively;
- `tempfile` 3.27.0; and
- `thiserror` 2.

### Testing references

- `docs/rust-testing-with-rstest-fixtures.md` covers fixtures, `#[case]`
  tables, and `#[from]` renaming. Its version numbers are stale, so use
  `rstest` 0.28.
- `docs/rstest-bdd-users-guide.md` covers feature files under
  `tests/features/`, `#[scenario(path = ..., name = ...)]` or `scenarios!`,
  `{name:Type}` placeholders, and steps that return `Result`. Its examples say
  0.2, so check the 0.6 API against existing repository tests under
  `cli/tests/`.
- `docs/developers-guide.md`, lines 290 to 315: `panic_in_result_fn` is
  denied, so assertions belong in `then` steps that return `()`, and `given` and
  `when` steps return `Err` rather than asserting. Clippy counts step
  parameters towards `too_many_arguments`. `clippy.toml`'s
  `allow-expect-in-tests` does not cover helpers under `tests/`.
- `docs/rust-doctest-dry-guide.md`: doctests compile as separate crates,
  fallible examples use `fn main() -> Result<...>`, and complex or private
  examples belong in unit tests.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`: avoid the
  bumpy-road shape. The pipeline is a flat sequence of named stage functions,
  not one function with a nested block per stage.

### Design references

- `docs/whitaker-dylint-suite-design.md` is the suite design. It has no
  rewrite section, so this item does not change it.
- `docs/whitaker-clone-detector-design.md`, lines 240 to 252 and 518 to 522,
  sets out the existing byte-span discipline: half-open, in bounds, UTF-8
  aligned, and typed errors instead of panics. Lines 667 to 674 are the
  precedent for confining a syntax library to one adapter file.
- `docs/ownership-shape-lints-design.md` §12 to §14 describes the Phase 9
  ownership lints that fixtures 1 and 2 imitate.
- `docs/adrs/adr-0003-formal-proof-strategy-for-the-clone-detector-pipeline.md`
  sets the proof strategy: Verus for small algebraic invariants, Kani for
  bounded execution, and tests for broad behaviour.

### Skills to load during implementation

Load `rust-router` first, then the skill for each task:

- `rust-types-and-apis` for the contract surface and newtypes;
- `rust-errors` for `SpikeError`, `PlanViolation`, and the harness errors;
- `rust-unit-testing` for `rstest`, `googletest`, `pretty_assertions`, and
  `insta`;
- `proptest` for the oracle properties;
- `verus` for `EP-M2`;
- `hexagonal-architecture` to keep the contract free of adapters;
- `domain-cli-and-daemons` for the `rewrite-spike` binary and subprocess
  handling;
- `nextest` for the spike's nextest configuration; and
- `nll-to-polonius` if `bor001` needs revisiting.

## Conformance basis

Upstream artefacts, all inspected at `40a85aa5`:

- ADR 0007, `docs/adrs/adr-0007-rust-rewrite-materialization-substrate.md`
  (status Proposed, dated 2026-08-30), especially §Technical requirements,
  §Repository placement, §Shared spike interface, §Primary rewrite fixtures,
  §Adversarial fixtures, §Validation pipeline, §Mandatory gates, and §Migration
  plan steps 1 and 2.
- RFC 0007: §6.2 (the edit model and its invariants), §Capability probing,
  and §10.2.
- RFC 0008: §5.2 to §5.3 (the intent and anchor model), the BOR table, and
  the `remove_then_reinsert` suppressions.
- RFC 0009: §7.1 to §7.3 (anchors, staleness, macros), §8.2 to §8.3
  (confidence and recipe kinds), and §9 (materializer preconditions).
- `docs/roadmap.md` item 14.1.1.
- `AGENTS.md` (code, testing, and documentation rules) and
  `docs/documentation-style-guide.md`.

There is no terms-of-reference document and no separate technical design for
the spike. ADR 0007 plays the technical-design role.

Requirements derived from those artefacts:

- `REQ-ISOLATION`: the spike is a standalone workspace that is not a root
  member, and adds neither candidate library (ADR 0007 §Repository placement).
- `REQ-CONTRACT`: both backends implement one backend-neutral `SpikeBackend`
  trait and edit model, with no backend types (ADR 0007 §Shared spike interface
  and §Technical requirements).
- `REQ-CAPTURE`: captures carry compiler identity, node identity, expansion
  provenance, file digest, a half-open range, snippet digest, and expected
  syntax. Macro-expansion-only captures are rejected before any edit. Range
  validity is checked before slicing or hashing. The materializer rejects stale
  digests with typed errors (ADR 0007 §Shared spike interface).
- `REQ-HARNESS`: the harness owns fixture snapshot integrity, sorting,
  overlap detection, application, diffs, compiler invocation, and report
  serialization. It passes backends only the declared sources, and contains no
  fixture-specific replacement (ADR 0007 §Shared spike interface and §Mandatory
  gates).
- `REQ-FIXTURES`: four primary fixtures, each with exact expected per-file
  edit ranges, expected outputs, and the compiler outcomes ADR 0007 states for
  it (ADR 0007 §Primary rewrite fixtures).
- `REQ-REJECTION`: rejection fixtures declare a typed error and stable
  diagnostic data, and the pipeline stops after asserting them (ADR 0007
  §Adversarial fixtures and §Validation pipeline step 3).
- `REQ-PIPELINE`: the 11-step validation pipeline, including fresh backend
  processes, exact range-set comparison, untouched-byte checks, formatting
  diagnostics, and baseline and candidate profile outcomes (ADR 0007
  §Validation pipeline).
- `REQ-DETERMINISM`: exactly 100 fresh backend processes per fixture.
  Successful runs must produce byte-identical raw plans. Rejections must
  produce identical typed errors and stable diagnostic data (ADR 0007
  §Validation pipeline and §Mandatory gates).
- `REQ-REPORT`: a stable, machine-readable JSON report (ADR 0007
  §Validation pipeline step 11 and §Spike exit artefacts).

Gaps between the baseline and the target:

- `GAP-TYPES`: ADR 0007 names types it never defines.
- `GAP-PLAN-SEMANTICS`: the RFCs leave same-start, no-op, and granularity
  semantics open, and RFC 0007 requires sorted input, which ADR 0007
  contradicts.
- `GAP-NLL-FLAG`: the pinned nightly rejects `-Zpolonius=off`.
- `GAP-GATES`: a standalone workspace is invisible to every gate, and three
  Make targets reuse the `test` recipe.
- `GAP-EOL`: `.gitattributes` would normalize fixture line endings.
- `GAP-RECIPE-KIND`: RFC 0009 lists no kind for `remove_then_reinsert`.

Trace links:

```plaintext
ADR0007 §Repository placement -> REQ-ISOLATION -> D2, D10 -> EP-M0, EP-M1
    -> dependency-rule test; cargo tree check
ADR0007 §Shared spike interface -> REQ-CONTRACT, GAP-TYPES -> D2, D7, D8 -> EP-M1
    -> spike_contract unit tests
ADR0007 §Shared spike interface -> REQ-CAPTURE -> D3 -> EP-M1, EP-M4b
    -> capture tests; rejection fixtures; double-stale-blind control
RFC0007 §6.2 -> REQ-HARNESS, GAP-PLAN-SEMANTICS -> D4, D9 -> EP-M2
    -> plan properties; exhaustive small-plan test; verus/rewrite_edit_plan.rs
ADR0007 §Primary rewrite fixtures -> REQ-FIXTURES, GAP-NLL-FLAG, GAP-EOL -> D5, D6, D13 -> EP-M3
    -> make spike-rewrite-fixtures; fixture_corpus.feature
ADR0007 §Validation pipeline -> REQ-DETERMINISM -> D11 -> EP-M4a
    -> determinism tests with double-replay and double-hash-order
ADR0007 §Validation pipeline -> REQ-PIPELINE, REQ-REJECTION, REQ-REPORT -> D6, D11 -> EP-M4b
    -> backend_conformance.feature; report snapshots
GAP-GATES -> D10 -> EP-M1 -> make check-fmt, make lint, and make test run the spike; lane contract tests
GAP-RECIPE-KIND, GAP-PLAN-SEMANTICS, GAP-NLL-FLAG -> D12 -> EP-M5 -> ADR 0007 amended; RFC 0007 note
```

## Verification plan

### Abstract model

A source map `S` is a finite map from workspace-relative paths to byte strings
that are valid UTF-8. An intent `I` names a recipe, a set of declared files
(each with a digest), and a sequence of captures. A capture `c` has a name, a
path `c.path`, a half-open range `[c.start, c.end)`, a file digest `c.base`, a
snippet digest `c.snip`, a provenance value, and an expected syntax.

`prepare(S, I)` either returns validated captures or the first error, in a
fixed precedence order:

1. Intent structure:
   1. The recipe is known.
   2. The declared files are non-empty and unique.
   3. Every capture name is well formed and unique.
   4. Each role's indices run from 0 with no gaps.
   5. Every capture path is declared.
   6. Each capture's `base` equals its file's declared digest.
   7. One compiler identity is used throughout.
   8. Each role's count and expected syntax match the recipe schema.
2. Then, for each capture in intent order:
   1. Provenance is acceptable. Macro-expansion-only is always rejected; an
      editable macro call site is rejected unless the recipe opts in.
   2. The path exists in `S`.
   3. `sha256(S[path]) = c.base`.
   4. `c.start <= c.end <= len(S[path])`, with both offsets on character
      boundaries.
   5. `sha256(S[path][c.start..c.end]) = c.snip`.

A file edit list `E = [(s_0, e_0, r_0), ..., (s_{n-1}, e_{n-1}, r_{n-1})]` over
a body `B` is well formed, written `WF(B, E)`, when all of the following hold:

- for every `i`: `s_i <= e_i <= len(B)`, both offsets are character
  boundaries, and `r_i != B[s_i..e_i]`;
- for every `i < n - 1`: `s_i < s_{i+1}` and `e_i <= s_{i+1}`.

The specification of application, `spec_apply`, is defined by recursion on the
last edit:

- `spec_apply(B, []) = B`; and
- `spec_apply(B, E ++ [(s, e, r)]) = spec_apply(B[0..s], E) ++ r ++ B[e..len(B)]`.

Canonicalization sorts by `(s, e, r)` and then checks `WF`. It rejects an input
that is not `WF` after sorting, reporting the first violation by canonical
index, with ties broken by a fixed order of violation kinds.

### Invariants and lemmas

- `INV-CAPTURE-PRECEDENCE` (`REQ-CAPTURE`): `prepare` returns exactly the
  error for the first failing check in the precedence order above. It never
  slices or hashes a range before validating it.
- `INV-NO-PANIC` (`REQ-CAPTURE`, `REQ-HARNESS`): `prepare`, `canonicalize`,
  and `apply` return typed errors and never panic on any input. This includes
  out-of-bounds, reversed, and mid-character ranges, and offsets near
  `u64::MAX`.
- `INV-CANONICAL-ORDER` (`REQ-HARNESS`): canonicalization is independent of
  input order. Every permutation of the same edits yields the same canonical
  plan, or the same first violation.
- `INV-WF-SOUND-COMPLETE` (`REQ-HARNESS`): canonicalization accepts exactly
  the edit lists that are pairwise disjoint under `D4`, in bounds, on
  boundaries, free of no-ops, and free of shared starts.
- `INV-APPLY` (`REQ-PIPELINE`): for `WF(B, E)`, the production
  `apply_canonical_plan` returns `spec_apply(B, E)` for that file.
- `INV-FRAME` (`REQ-PIPELINE`): for `WF(B, E)`, define the shift before gap
  `k` as `δ_k = Σ_{j<k} (|r_j| - (e_j - s_j))`. Every byte `B[p]` in gap `k`
  (between `e_{k-1}` and `s_k`) appears at output position `p + δ_k`.
  `untouched_bytes_preserved` checks exactly this mapping.
- `INV-DETERMINISM` (`REQ-DETERMINISM`): for a deterministic backend, the
  outcome bytes and the exit status are identical across 100 fresh processes,
  and the harness rejects any difference. Infrastructure failures are reported
  as `Inconclusive`, not as divergence.
- `INV-FIXTURE-CONSISTENCY` (`REQ-FIXTURES`): for every success fixture,
  applying the manifest's declared edits to `before/` yields `after/` byte for
  byte. Every capture's literal text, range, and digests agree. `after/` adds
  no warning code that `before/` lacks.
- `INV-SNAPSHOT-ONLY` (`REQ-HARNESS`): pipeline step 1 reads fixture bytes and
  file digests only, and never reads captures.
- `L-ADJACENT-IMPLIES-PAIRWISE`: if starts are strictly increasing and each
  adjacent pair satisfies `e_i <= s_{i+1}`, then `e_i <= s_j` and `s_i < s_j`
  for all `i < j`. This lemma justifies checking adjacent pairs only, so it
  carries `INV-WF-SOUND-COMPLETE`.
- `L-FORWARD-EQUALS-SPEC`: the forward-cursor algorithm
  `forward_apply(B, E, cursor)` equals `spec_apply` for every `WF(B, E)`. The
  algorithm appends `B[cursor..s_i] ++ r_i` for each edit, then appends
  `B[e_last..len(B)]`. This lemma relates the production shape to the
  end-to-start formulation in RFC 0007 §10.2.
- `L-OFFSET-MAP`: for `WF(B, E)` and every byte position `p` outside all
  ranges, `spec_apply(B, E)[p + δ(p)] = B[p]`, with `δ` as in `INV-FRAME`.

These lemma statements are hypotheses about the proof's shape. The `EP-M2` work
may merge, split, or replace them once the verification conditions are
concrete. Statements stay here, unproved, until then.

### Axioms (trusted, not verified here)

- `AXIOM-SHA256`: `sha2::Sha256` computes SHA-256 correctly, and collisions do
  not occur in practice. Capture and fixture digest checks depend on this.
- `AXIOM-STR-BOUNDARY`: `str::is_char_boundary` and `str::get` agree with
  UTF-8 character boundaries. Range validation depends on this.
- `AXIOM-RUSTC`: the pinned `rustc` decides NLL and `-Zpolonius=next`
  acceptance. The compiler is the oracle for fixture outcomes; the spike does
  not reason about borrow checking itself.
- `AXIOM-CARGO`: Cargo honours `CARGO_ENCODED_RUSTFLAGS`, `CARGO_TARGET_DIR`,
  and `--offline` as documented. A package with an empty `[workspace]` table is
  its own workspace root. `--message-format=json` reports `compiler-artifact`,
  `compiler-message`, and `build-finished` messages.
- `AXIOM-PROCESS`: a fresh operating-system process gets a fresh `RandomState`
  seed and shares no state with earlier runs. 100 fresh processes can therefore
  expose ordering nondeterminism that a single process would hide.
- `AXIOM-SERDE`: `serde_json` serializes struct fields in declaration order and
  sequences in element order.

### Methods per obligation

- Obligation: `INV-CAPTURE-PRECEDENCE`.
  - Method: an `rstest` table, plus a construction-based `proptest`
    property.
  - Rationale: the precedence order is a finite decision list, so a table
    names each rule. The property checks combinations of faults that the table
    would miss.
  - Domain: the table has one row per error class. The property injects zero
    to three non-interfering faults into a valid synthetic intent. For
    example, it changes a range without changing the file. The expected error
    is the injected fault with the highest precedence.
  - Artefacts: `crates/spike_contract/src/capture/tests.rs` and
    `crates/spike_contract/tests/capture_precedence.rs`.
  - Evidence: run the focused `spike_contract` `capture` filter (see
    `Concrete steps`). The tests fail before `prepare_intent` exists and pass
    afterwards.
  - Non-vacuity:
    - The oracle comes from the construction, not from a second copy of the
      rules.
    - Class coverage is counted in local state, using a manually driven
      `TestRunner` with a fixed seed. Every fault class must appear at least
      once.
    - A seeded fault that swaps the file-digest and range checks must fail the
      property on a case that has both faults.

- Obligation: `INV-NO-PANIC`.
  - Method: `proptest`.
  - Rationale: panics hide in input combinations nobody tested, and generated
    input finds them cheaply.
  - Domain: source strings mixing ASCII with multi-byte characters (`é`, `日`,
    and `🦀`). Offsets are drawn from `0..=len + 4` and from near `u64::MAX`.
    Each case runs `prepare`, then canonicalize, then apply.
  - Artefact: `crates/spike_plan/tests/no_panic.rs`.
  - Evidence: the property passes. The `indexing_slicing` and `string_slice`
    denials prevent unchecked slicing at compile time.
  - Non-vacuity: a counter shows that mid-character offsets were generated. A
    seeded fault that replaces `str::get` with direct slicing must panic under
    the property.

- Obligation: `INV-CANONICAL-ORDER` and `INV-WF-SOUND-COMPLETE`.
  - Method: `proptest` against an independent O(n²) pairwise oracle, plus a
    bounded exhaustive test.
  - Rationale: the space of edit lists is large and sensitive to order. The
    pairwise oracle shares no code with the adjacent-pair implementation. The
    exhaustive test closes the gap between the Verus model and the compiled
    code within a stated bound.
  - Domain:
    - The property generates zero to eight edits over a body of up to 64
      bytes. It is biased towards touching, nested, same-start, zero-width,
      identity, and duplicate-range edits, and shuffles the input order.
    - The exhaustive test covers every list of up to three edits over a
      four-byte body, with ranges inside `0..=4` and replacements `""`, `"x"`,
      or the original bytes.
  - Artefacts: `crates/spike_plan/tests/plan_properties.rs` and
    `crates/spike_plan/tests/exhaustive_small_plans.rs`.
  - Evidence: both pass. The recorded class counts show every class was
    reached: empty plan, touching edits, zero-width edit, same-start conflict,
    strict overlap, out of bounds, mid-character, no-op, identity, and
    duplicate range.
  - Non-vacuity: two seeded faults must each be rejected, then both are
    reverted and their transcripts recorded:
    - Allowing `e_i = s_{i+1} + 1` must break the oracle comparison.
    - Removing the same-start rule must break it on a zero-width pair.

- Obligation: `INV-APPLY` and `INV-FRAME`.
  - Method: `proptest` against an end-to-start oracle, plus a direct unit
    test of the untouched-byte checker.
  - Rationale: the oracle applies edits from last to first with
    `String::replace_range`, the formulation in RFC 0007 §10.2. It shares no
    code with the production forward cursor. The pipeline's untouched-byte
    check cannot fail once range sets match, so it needs a direct test.
  - Domain: well-formed plans from the generator above.
  - Artefact: `crates/spike_plan/tests/plan_properties.rs`.
  - Evidence: the properties pass. `untouched_bytes_preserved` returns false
    for a hand-corrupted output, naming the first differing offset.
  - Non-vacuity: a seeded off-by-one in the cursor update must fail the
    property.

- Obligation: `L-ADJACENT-IMPLIES-PAIRWISE`, `L-FORWARD-EQUALS-SPEC`, and
  `L-OFFSET-MAP`.
  - Method: Verus deductive proof.
  - Rationale: these statements are unbounded in both the number of edits and
    the body length. They carry the guarantees the harness relies on when it
    checks only adjacent pairs, applies edits in one forward pass, and checks
    untouched bytes through an offset map.
  - Domain: `Seq<u8>` bodies and `Seq<Edit>` plans with `nat` offsets. UTF-8
    boundaries are not modelled. They are a separate check whose correctness
    rests on `AXIOM-STR-BOUNDARY`.
  - Artefacts:
    - `verus/rewrite_edit_plan.rs`;
    - a new `rewrite-edit-plan` group in `scripts/run-verus.sh`, also added to
      `all`; and
    - a Makefile target, `verus-rewrite-edit-plan`.
  - Evidence: `make verus-rewrite-edit-plan` prints
    `verification results:: N verified, 0 errors`, where `N` is recorded at
    implementation time.
  - Non-vacuity:
    - Witness: the file contains a witness lemma. It builds a concrete plan
      over `"abcdef"` that satisfies `WF`, with a replacement, a zero-width
      insertion, and a pair of touching edits, and proves the expected output.
      This shows the precondition can be met.
    - Independence: `spec_apply` is defined by last-edit-first recursion and
      `forward_apply` by a forward cursor, so `L-FORWARD-EQUALS-SPEC` relates
      two different definitions rather than restating one.
    - Seeded fault: weaken the hypothesis of `L-ADJACENT-IMPLIES-PAIRWISE` to
      `e_i <= s_{i+1} + 1`, leaving the conclusion unchanged. The lemma must
      then fail to verify. Record the transcript, then revert.
    - No shortcuts: the proof contains no `assume`, `admit`, or
      `external_body`. Running `grep -nE 'assume|admit|external_body'
      verus/rewrite_edit_plan.rs` and getting no output is part of the
      evidence.

- Obligation: `INV-DETERMINISM`.
  - Method: an integration test with real processes and test doubles.
  - Rationale: nondeterminism across processes is exactly what is being
    detected, so only real processes can exercise it (`AXIOM-PROCESS`).
  - Domain: the 100-run gate executes only in this test binary:
    - `double-replay` on the four primary fixtures and on the stale-snippet
      rejection fixture, 100 runs each;
    - `double-hash-order`, which emits eight edits in `HashSet` iteration
      order, on one fixture;
    - `double-flaky-rejection`, which blames a different capture in different
      processes, on the rejection fixture; and
    - `double-hanging` and `double-flooding`, at three runs each, for the
      `Inconclusive` path.
  - Artefact: `crates/spike_cli/tests/determinism.rs`.
  - Evidence:
    - `double-replay` agrees across all 100 runs on every fixture.
    - `double-hash-order` fails with `OutcomeDiverged { first_run: 0,
      diverging_run: k }`.
    - `double-flaky-rejection` fails at the same gate.
    - `double-hanging` and `double-flooding` are reported as `Inconclusive`
      with their cause, and never as `OutcomeDiverged`.
  - Non-vacuity:
    - `double-hash-order` is the negative control. With eight keys, the chance
      that 100 runs agree by luck is negligible.
    - The test also asserts that `double-replay`'s plans are non-empty, so
      the comparison is never between empty outputs.

- Obligation: `INV-FIXTURE-CONSISTENCY` and `INV-SNAPSHOT-ONLY`.
  - Method: a manifest-driven corpus check, plus a structural test of the
    pipeline's first stage.
  - Rationale: the fixtures are the oracle for 14.1.2. If they are wrong,
    both backends are judged against a wrong answer.
  - Domain: the four primary fixtures, and the test-only rejection fixtures
    for the snapshot stage.
  - Artefacts:
    - `crates/spike_harness/src/fixture/corpus_check.rs`;
    - `crates/spike_harness/src/fixture/snapshot.rs`; and
    - `crates/spike_cli/tests/features/fixture_corpus.feature`.
  - Evidence: `make spike-rewrite-fixtures` passes. Each of the following
    corruption tests fails, naming the file, the offset, and the bytes
    actually found:
    - one byte of `after/` is changed;
    - a capture's literal text is changed; and
    - a fixture is checked out with the wrong line endings.

    The snapshot stage passes a rejection fixture whose snippet digest is
    deliberately stale.
  - Non-vacuity: the corruption tests are the negative controls. The stale
    rejection fixture passing the snapshot stage shows that stage reads no
    captures.

- Obligation: the expected compiler outcome for each fixture.
  - Method: real compiler runs through the profile runner, with a fake
    executor for the runner's own logic.
  - Rationale: `AXIOM-RUSTC`. Only the compiler can confirm acceptance. The
    flag selection, outcome parsing, and `Inconclusive` handling are pure
    logic, so a fake executor tests them.
  - Domain:
    - real compiler: each fixture's `before/` and `after/` under both
      profiles, plus the control and canary packages;
    - fake executor: both toolchain shapes (NLL by default, and NLL needing
      `-Zpolonius=off`), the `unavailable` case, a missing
      `compiler-artifact` message, and a deadline hit.
  - Artefacts: `crates/spike_harness/src/profile/` with its unit tests, and
    `crates/spike_cli/tests/compiler_outcomes.rs` (the only real-compiler
    test binary).
  - Evidence: `make spike-rewrite-fixtures` prints the outcome table shown in
    `Purpose / big picture`.
  - Non-vacuity:
    - The canary must be rejected by `nll` and accepted by `polonius-next`
      before any fixture result counts. This shows the two profiles really
      differ on this host.
    - `bor001`'s `after/` must be rejected by NLL, which shows `nll` is not
      silently running Polonius.
    - A `rustc -vV` mismatch, injected through the fake executor, must make
      both profiles `unavailable`.

### Formal verification decision

Verus is used for the edit-plan kernel only (`D9`). The kernel is contractual
logic that every later rewrite stage reuses: RFC 0007 §10.2 and RFC 0009 §20.2
both name non-overlap and faithful application as required properties. The
harness's design also rests on three non-obvious claims: checking adjacent
pairs is enough, forward application matches the specification, and the
untouched-byte check's offset map is correct. All three are unbounded in the
number of edits, so tests can only sample them. A small pure model exists, and
the repository already has a closely related proof
(`verus/string_continuation_rewrite.rs`).

As with the repository's other Verus files, the proof covers an independent
model, not the compiled Rust. Two kinds of test close the gap between model and
code. The `proptest` oracle properties run the real functions over generated
inputs. The bounded exhaustive test runs them over every plan within its bound.

Kani is not used. Its strength here would be a bounded, exhaustive check of the
compiled `canonicalize_plan` for small plans. The exhaustive enumeration test
gives the same coverage of the compiled code without another tool, and the
Verus proof covers the unbounded statement. When 14.2.1 lifts the kernel into
production code, it must revisit Kani.

No proof obligations are assigned to capture validation, fixture loading,
process management, compiler invocation, diffs, or the report. These are finite
decision lists or integration plumbing. Tables, properties, and end-to-end
tests cover them in proportion to their risk.

### Suggested BDD features

Two feature files drive `EP-M3` and `EP-M4b`. `rstest-bdd` 0.6 runs both, from
`crates/spike_cli/tests/`. Both use `--runs 3`. The full 100-run gate runs only
in `determinism.rs`.

`spikes/rewrite-materialization/crates/spike_cli/tests/features/fixture_corpus.feature`
(`REQ-FIXTURES`, `EP-M3`):

```gherkin
Feature: Shared rewrite fixture corpus
  Scenario: Every primary fixture is internally consistent
    Given the primary fixture corpus
    When the corpus check runs
    Then every fixture's declared edits reproduce its expected output exactly
    And every capture's text, range, and digests agree

  Scenario: A corrupted expected output is reported precisely
    Given a copy of fixture "own001-clone-only-borrow" whose expected output has one changed byte
    When the corpus check runs
    Then the check fails naming file "src/lib.rs" and the differing offset

  Scenario: The checker profiles are distinguishable on this toolchain
    Given the pinned toolchain
    When the capability probe runs
    Then the control package is accepted by both profiles
    And the nll profile rejects the canary with exactly "E0499"
    And the polonius-next profile accepts the canary

  Scenario: The conditional lookup fixture is Polonius-only
    Given fixture "bor001-conditional-lookup"
    When both checker profiles compile its expected output
    Then nll rejects it with error code "E0499"
    And polonius-next accepts it
```

`spikes/rewrite-materialization/crates/spike_cli/tests/features/backend_conformance.feature`
(`REQ-CAPTURE`, `REQ-REJECTION`, `REQ-PIPELINE`, `EP-M4b`):

```gherkin
Feature: Backend conformance pipeline
  Scenario: A faithful backend passes every primary fixture
    Given the double-replay backend
    When the pipeline runs every primary fixture
    Then every fixture passes every gate
    And the report records confidence "review_required" for "bor002-remove-reinsert"

  Scenario: A faithful backend rejects a stale capture identically
    Given the double-replay backend
    And the rejection fixture "own001-stale-snippet"
    When the pipeline runs that fixture
    Then the fixture passes with diagnostic key "spike.capture.stale.snippet" for capture "binding"
    And the pipeline stopped after comparing outcomes

  Scenario: A backend that ignores a stale capture is rejected
    Given the double-stale-blind backend
    And the rejection fixture "own001-stale-snippet"
    When the pipeline runs that fixture
    Then the fixture fails because a plan was produced where a rejection was expected

  Scenario: A backend that reports the wrong stale kind is rejected
    Given the double-wrong-kind backend
    And the rejection fixture "own001-stale-snippet"
    When the pipeline runs that fixture
    Then the fixture fails because the diagnostic key differs from the manifest

  Scenario: A backend that widens an edit is rejected before application
    Given the double-widening backend
    When the pipeline runs fixture "own002-owned-param"
    Then the fixture fails because the edit-range set differs from the manifest
    And no rewritten source was produced

  Scenario: Overlapping edits are detected before application
    Given the double-overlapping backend
    When the pipeline runs fixture "bor002-remove-reinsert"
    Then the fixture fails with a shared-start conflict between two edits in one file

  Scenario: A backend is given only the declared source files
    Given the double-source-auditor backend
    When the pipeline runs fixture "own002-owned-param"
    Then the backend reports that it received exactly the three declared files

  Scenario: A crashing backend is reported, not propagated
    Given the double-panicking backend
    When the pipeline runs fixture "own001-clone-only-borrow"
    Then the fixture fails because the backend process exited abnormally
    And the harness itself exits normally
```

## Plan of work

The work runs in seven milestones. Each ends with the full gate sequence in
`Concrete steps`. The code milestones follow Red-Green-Refactor:

1. Red: write the failing test first, and run it to confirm it fails for the
   stated reason.
2. Green: write the smallest code that passes it.
3. Refactor: tidy the code and rerun.

### `EP-M0` workspace and inheritance probe (prototype)

Question: does a standalone workspace under `spikes/` build cleanly, and which
root configuration does it inherit? Build a throwaway workspace under
`target/scratch/spike-probe/`. Give it one library package and one
fixture-style package with an empty `[workspace]` table, then record the answer
to each of these checks:

1. `cargo metadata --manifest-path <probe>/Cargo.toml --no-deps` succeeds and
   reports only the probe's packages.
2. `cargo fmt --manifest-path <probe>/Cargo.toml --all -- --check` applies
   the root `.rustfmt.toml`. Test this with a `use` statement that only
   `imports_granularity = "Crate"` would reformat.
3. Whether `cargo clippy` in the probe reads the root `clippy.toml`. Test this
   with an `expect` call inside a `#[cfg(test)]` function, which only
   `allow-expect-in-tests` would permit.
4. Whether `cargo nextest run` in the probe reads the root
   `.config/nextest.toml`. The expected answer is no.
5. A fixture-style package copied under `target/rewrite-spike/` builds with
   `cargo check --offline`, and Cargo does not report that it "believes it's in
   a workspace".
6. `rustc -vV`, run with `RUSTUP_TOOLCHAIN` removed from the environment and
   from inside the copy, reports commit `57d06900f`.

Write the transcript into `Artefacts and notes`, then delete the probe. Go on to
`EP-M1` only if checks 1, 2, 5, and 6 hold. If check 1 or 5 fails, stop and
escalate, because `D2` and `D6` depend on them. Checks 3 and 4 only decide
whether the spike's own `clippy.toml` and `.config/nextest.toml` are required
or merely explicit; both files are created either way.

### `EP-M1` spike workspace, gate wiring, contract model, and capture validation

Assigned: `REQ-ISOLATION`, `REQ-CONTRACT`, `REQ-CAPTURE`, `GAP-TYPES`,
`GAP-GATES`, `GAP-EOL`; decisions `D2`, `D3`, `D7`, `D8`, `D10`, `D11`.

1. Create the workspace configuration:
   - `spikes/rewrite-materialization/Cargo.toml`, with
     `[workspace] members = ["crates/*"]`, `resolver = "2"`, a
     `[workspace.package]` table (edition 2024, `publish = false`, licence
     ISC), `[workspace.dependencies]` for the authorized crates, and
     `[workspace.lints]` tables copied from the root `Cargo.toml`.
   - `spikes/rewrite-materialization/clippy.toml`, holding the root settings
     plus `disallowed-types`.
   - `spikes/rewrite-materialization/.config/nextest.toml`, defining `default`
     and `ci` profiles, each with a 300-second `slow-timeout`, and a
     `real-compiler` test group with `max-threads = 1`, a 75-minute allowance,
     and no retries.
2. Create all five packages as empty, documented crates, so the dependency
   rule is in force from the first commit. `spike_cli`'s binary only parses its
   subcommands and prints usage.
3. Red: write the guard tests, each in `crates/spike_cli/tests/`:
   - `lint_policy.rs` parses the root and spike manifests and `clippy.toml`
     files with `toml`. It asserts that every root lint is present in the
     spike at the same level or stricter, and that every root `clippy.toml`
     threshold is equal in the spike. Run it before copying the tables, and
     see it fail.
   - `dependency_rule.rs` parses every `crates/*/Cargo.toml` and asserts the
     dependency graph from `Constraints`, including that no package lists
     `ast-grep` or a `ra_ap_` crate.
4. Red: write unit tests for the newtypes in `spike_contract`:
   - `WorkspaceRelativePath::new` rejects absolute paths, `..`, empty strings,
     backslashes, and trailing separators, and accepts `src/lib.rs`.
   - `Sha256Digest` parses and prints 64 lower-case hexadecimal characters,
     and rejects anything else.
   - `CaptureName` parses `<role>` and `<role>.<index>` and rejects other
     forms.
   - `ByteRange::validate_in(source)` returns
     `RangeFault::{Reversed, OutOfBounds, StartNotCharBoundary,
     EndNotCharBoundary}`.
5. Green: implement the model types listed under `Interfaces and dependencies`
   in `crates/spike_contract/src/model/`. The contract depends on `serde` with
   `derive`, but not on `serde_json` or `toml`.
6. Red: write the `INV-CAPTURE-PRECEDENCE` table and property against
   `prepare_intent`, using a test-only recipe schema defined in the tests.
7. Green: implement `prepare_intent`, `PreparedIntent`, `ValidatedCapture`,
   and the `RecipeSchema` mechanism in `crates/spike_contract/src/capture/` and
   `crates/spike_contract/src/recipe/`. The four primary recipes' schemas are
   added in `EP-M3`, once the fixtures pin down the roles. That avoids writing
   them now and reworking them later.
8. Add `SpikeBackend`, `SpikeError`, `IntentFault`, and `StableDiagnostic`
   in `crates/spike_contract/src/backend.rs` and
   `crates/spike_contract/src/error/`. Every `SpikeError` variant maps to one
   dotted key, such as `spike.capture.stale.snippet`, through a single `match`.
   The full key table is written down in a test and treated as append-only.
9. Wire the gates (`D10`):
   - Add the Makefile targets and the `SPIKE_REWRITE_TEST` switch.
   - Make `coverage-build` and `trybuild-contracts` pass
     `SPIKE_REWRITE_TEST=0`.
   - Extend `fetch` to fetch the spike's dependencies.
   - Add the line `spikes/rewrite-materialization/fixtures/** -text` to
     `.gitattributes`, after the `*.rs` rule, with a comment explaining it.
   - Add `spikes/rewrite-materialization/fixtures/` to the `[files] exclude`
     list in `typos.local.toml`.
   - Extend `tests/workflow_contracts/lane_deduplication_contract_test.py`:
     add the spike command patterns to `TEST_COMMANDS`, and add a test that
     `coverage-build` and `trybuild-contracts` pass `SPIKE_REWRITE_TEST=0`.
   - Run `make test-workflow-contracts`.
10. Refactor: keep every file under 400 lines.

Acceptance:

- The contract, lint-policy, and dependency-rule tests pass.
- `make check-fmt`, `make lint`, and `make test` each run their spike target
  and pass.
- `make test-workflow-contracts` passes.
- The dependency-tree check prints nothing:

  ```bash
  cargo tree --manifest-path spikes/rewrite-materialization/Cargo.toml | grep -E 'ast-grep|ra_ap'
  ```

- The root `cargo metadata --no-deps` lists no spike package.

### `EP-M2` edit-plan kernel and Verus proofs

Assigned: `REQ-HARNESS` (sorting, overlap detection, application, range-set
comparison, and the untouched-byte check), `GAP-PLAN-SEMANTICS`; decisions `D4`
and `D9`.

The kernel lives in `spike_plan`. Only `spike_harness` depends on it. It imports
`ByteRange`, `WorkspaceRelativePath`, `Sha256Digest`, `TextEdit`, `FileEdits`,
`TextEditPlan`, and `SourceMap` from the contract, and nothing else, so 14.2.1
can lift it as one unit.

1. Red: write `rstest` tables for `canonicalize_plan`, covering:
   - each violation: `OutOfBounds`, `NotCharBoundary`, `NoOpEdit`,
     `SharedStart`, `Overlap`, `DuplicateFile`, `EmptyFileEdits`,
     `UndeclaredFile`, and `SourceMissing`;
   - each accepted shape: the empty plan, touching edits, a zero-width
     insertion followed by a later replacement, and unsorted input that
     becomes sorted.
2. Red: write the `INV-NO-PANIC`, `INV-CANONICAL-ORDER`,
   `INV-WF-SOUND-COMPLETE`, `INV-APPLY`, and `INV-FRAME` properties, together
   with their oracles and the exhaustive small-plan test.
3. Green: implement `canonicalize_plan`, `CanonicalPlan`,
   `CanonicalFileEdits`, `apply_canonical_plan`, `edit_range_set`, and
   `untouched_bytes_preserved` in `crates/spike_plan/src/`.
   `apply_canonical_plan` takes the `SourceMap` and checks each file's digest
   against the one recorded in the canonical plan.
4. Write `verus/rewrite_edit_plan.rs`, containing `spec_apply` (last-edit-first
   recursion), `forward_apply` (a forward cursor), the three lemmas, and the
   witness. Then:
   - add the `rewrite-edit-plan` group to `scripts/run-verus.sh`, including in
     `all`;
   - add the `verus-rewrite-edit-plan` Makefile target; and
   - run the seeded faults described in `Verification plan`, and record their
     transcripts.

   If proof time grows, follow `docs/execplans/3-5-1-root-whitaker-binary.md`:
   prefer plain equalities to symbolic `by (compute)`.
5. Refactor: keep the shape of the production forward cursor identical to
   `forward_apply`. Give each a doc comment that points at the other.

Acceptance:

- The properties and the exhaustive test pass, and their class counts are
  recorded.
- `make verus-rewrite-edit-plan` and `make verus` pass.
- The seeded-fault transcripts are recorded in `Artefacts and notes`.
- The standard gates pass.

### `EP-M3` primary fixtures, corpus check, and checker profiles

Assigned: `REQ-FIXTURES`, `GAP-NLL-FLAG`, `GAP-EOL`; decisions `D5`, `D6`, `D7`,
`D8`, `D13`.

Each fixture is a library crate whose public function exercises its private
code, so that it compiles without warnings.

#### Fixture `own001-clone-only-borrow`

Recipe metadata: `ownership.clone_only_used_by_borrow`, with
`confidence = "review_required"`, `may_change = ["allocation"]`,
`checker_requirement = "nll"`, and `signature_change = "none"`.

`before/src/lib.rs` contains, among the supporting items:

```rust
fn summarise(config: &Config) -> usize {
    let mut total = 0;
    let label = config.name.clone();

    // Measure before counting so totals line up.
    total += measure(&label);
    total += count_vowels(&label);
    total
}
```

- Captures: `binding` (the `let label` statement), `use.0` and `use.1` (the
  two `label` path expressions), and `source` (the place `config.name`).
- Edits:
  - delete the whole line `let label = config.name.clone();\n`; and
  - replace each `label` path with `config.name`.
- The blank line and the comment stay. The leading `let mut total` line means
  the deletion leaves no blank line straight after `{`.
- Outcomes: both sides accepted by both profiles.

#### Fixture `own002-owned-param`

Recipe metadata: `ownership.owned_param_causes_clone`, with
`confidence = "locally_reasoned"`, `signature_change = "private"`, and
`checker_requirement = "nll"`.

Files:

- `src/lib.rs` declares a private `fn render_label(label: String) -> usize`
  with a doc comment and an `#[inline]` attribute, plus `mod alpha;` and
  `mod beta;`.
- `src/alpha.rs` has `use crate::render_label;`, one unrelated `use`, and one
  call `render_label(name.clone())`.
- `src/beta.rs` calls `crate::render_label(item.name.clone())` twice.

Captures: `param_type` (the `String` in the signature) and `clone_arg.0` to
`clone_arg.2` (each `.clone()` argument).

Edits:

- `String` becomes `&str`;
- `name.clone()` becomes `&name`; and
- each `item.name.clone()` becomes `&item.name`.

Three files are edited. The doc comment, the attribute, and both `use` lines
are unchanged. Outcomes: both sides accepted by both profiles.

#### Fixture `bor001-conditional-lookup`

Recipe metadata: `borrow.lookup_then_relookup`, with
`confidence = "review_required"` and `checker_requirement = "polonius-next"`.

`before/` contains the lookup-then-relookup tail expression:

```rust
fn entry_or_default(map: &mut HashMap<u32, String>, key: u32) -> &mut String {
    if map.contains_key(&key) {
        // Existing entry: hand out the borrow directly.
        map.get_mut(&key).unwrap()
    } else {
        // Missing entry: insert, then borrow the new value.
        map.insert(key, String::new());
        map.get_mut(&key).unwrap()
    }
}
```

Captures: `region` (the whole tail `if` expression), `map`, `key`, and
`default` (the `String::new()` expression).

The single edit replaces the region with this, which keeps both branch comments
inside their branches and splices in the captured default:

```rust
match map.get_mut(&key) {
        Some(value) => {
            // Existing entry: hand out the borrow directly.
            value
        }
        None => {
            // Missing entry: insert, then borrow the new value.
            map.entry(key).or_insert(String::new())
        }
    }
```

The first line has no indentation because the region starts after the existing
indentation. Outcomes:

- `before/`: accepted by both profiles.
- `after/`: rejected by `nll` with exactly `{E0499}`, and accepted by
  `polonius-next`.

The re-probe in `Artefacts and notes` confirms this shape on the pinned nightly.

#### Fixture `bor002-remove-reinsert`

Recipe metadata: `borrow.remove_then_reinsert`, with
`confidence = "review_required"`, `may_change = ["map_order", "allocation"]`,
and `checker_requirement = "nll"`.

`before/` contains:

```rust
fn apply_increment(
    counters: &mut HashMap<String, Counter>,
    key: &str,
    raw: &str,
) -> Result<(), CounterError> {
    let increment: u32 = raw.parse().map_err(CounterError::Parse)?;
    let mut counter = counters.remove(key).ok_or(CounterError::Missing)?;
    // Saturate rather than wrap: counters are monotonic.
    counter.hits = counter.hits.saturating_add(increment);
    counters.insert(key.to_owned(), counter);
    Ok(())
}
```

Both `?` operators sit before or on the extraction statement, never between the
extraction and the reinsertion. That keeps the shape inside what RFC 0008
reports.

Captures: `extraction`, `mutation`, and `reinsertion`.

Edits:

- replace the extraction statement with
  `let counter = counters.get_mut(key).ok_or(CounterError::Missing)?;`, which
  drops the `mut`; and
- delete the whole reinsertion line.

The mutation and its comment stay byte-identical. The remaining semantic
differences are recorded in `may_change` and in the README, citing ADR 0007's
non-goal "Prove behavioural equivalence of remove/reinsert":

- map iteration order and allocation; and
- the reinsertion's `key.to_owned()` is no longer evaluated.

Outcomes: both sides accepted by both profiles, with no new warnings.

The `.unwrap()` calls in `bor001`'s `before/` are fixture data. The spike's
lints do not apply to fixture files.

#### Steps

1. Red: write `fixture_corpus.feature` and its step definitions, and the
   corpus-check unit tests, including the three corruption controls. Run them
   with no fixtures present, and see them fail.
2. Green: implement the fixture support in
   `crates/spike_harness/src/fixture/`:
   - `manifest.rs`: the manifest model, parsed with `toml`, which rejects
     multi-line strings;
   - `loader.rs`: reads the `before/` and `after/` trees into a `SourceMap`
     using `cap_std::fs_utf8`;
   - `snapshot.rs`: the pipeline-step-1 check of file bytes, file digests, and
     line endings only; and
   - `corpus_check.rs`: the authoring-time check.

   The corpus check never runs in the pipeline. For success fixtures it
   verifies:
   - every capture's range, digests, and literal text;
   - that applying the declared edits (through `canonicalize_plan` and
     `apply_canonical_plan`) reproduces `after/` exactly; and
   - that `after/` contains no file that `before/` lacks.

   On a mismatch it prints the bytes actually found at the declared range.
3. Add the four recipe schemas to `spike_contract/src/recipe/`, following the
   capture roles above, then author the four fixtures by hand.
4. Red: write unit tests for the profile runner. Use a fake command executor:
   a closure seam that returns canned exit statuses and JSON message streams.
   Cover:
   - flag selection for both toolchain shapes;
   - the `unavailable` outcome;
   - the `rustc -vV` commit check;
   - the requirement for a `compiler-artifact` message;
   - each `Inconclusive` cause; and
   - extraction of error and warning codes.
5. Green: implement `crates/spike_harness/src/profile/`. `ProfileRunner`
   works as follows:
   - It creates a per-invocation temporary directory with
     `tempfile::Builder::tempdir_in("<root>/target/rewrite-spike/runs")`.
   - It copies each package into a short path inside that directory, so that
     Windows path-length limits are not hit.
   - It runs `cargo check --offline --message-format=json` with a fresh
     `CARGO_TARGET_DIR` for each compile.
   - It starts the child's environment from the parent's, then removes
     `CARGO`, `RUSTUP_TOOLCHAIN`, `RUSTC`, `RUSTC_WORKSPACE_WRAPPER`, every
     `CARGO_BUILD_*` and `CARGO_PROFILE_*` variable, `CARGO_INCREMENTAL`,
     `CARGO_TARGET_DIR`, `LLVM_PROFILE_FILE`, `RUSTFLAGS`, `RUSTDOCFLAGS`, and
     `CARGO_ENCODED_RUSTFLAGS`.
   - It sets `CARGO_ENCODED_RUSTFLAGS` to the profile's flags, and keeps
     `RUSTC_WRAPPER` and `CARGO_NET_OFFLINE`.
   - It records the compile outcome together with the sorted, de-duplicated
     warning codes.
   - It caches outcomes by profile and package content digest for the rest of
     the invocation.
   - It writes the control and canary packages at run time.
   - A failure to clean up the temporary directory is a warning, not an
     error.
6. Wire up `rewrite-spike fixtures`, which runs the toolchain check, the
   probe, the corpus check, and the profile checks, then prints the outcome
   table. Add the `spike-rewrite-fixtures` Makefile target.

Acceptance:

- `make spike-rewrite-fixtures` prints the table shown in
  `Purpose / big picture` and exits zero.
- The BDD feature passes.
- The corruption controls fail as specified.
- The standard gates pass.

### `EP-M4a` process protocol, test backends, and determinism runner

Assigned: `REQ-DETERMINISM`; decision `D11`.

1. Red: write `crates/spike_cli/tests/determinism.rs`, the unit tests for the
   process runner, and the registry-completeness test, which checks that every
   double appears in the registry.
2. Green: implement the protocol, the doubles, the registry, and the runners:
   - `spike_contract::outcome`: `MaterializeOutcome` and its serialization
     contract.
   - `crates/spike_test_backends/`, which depends on `spike_contract` only.
     Each double is one small module. Most return the manifest's expected
     edits, which reach them through a test-only recipe payload. The
     behaviours are:

     | Double                   | Behaviour                                                                                                                    |
     | ------------------------ | ---------------------------------------------------------------------------------------------------------------------------- |
     | `double-replay`          | Calls `prepare_intent`, then returns the expected edits in reverse order, so the harness's sorting is exercised.             |
     | `double-stale-blind`     | Skips `prepare_intent`.                                                                                                      |
     | `double-wrong-kind`      | Reports a stale file where the snippet is stale.                                                                             |
     | `double-hash-order`      | Emits eight edits in `HashSet` order.                                                                                        |
     | `double-flaky-rejection` | Blames a capture chosen from a fresh `RandomState` hash.                                                                     |
     | `double-widening`        | Extends one edit by one byte.                                                                                                |
     | `double-overlapping`     | Duplicates one edit's start offset.                                                                                          |
     | `double-source-auditor`  | Returns `UnsupportedRecipeVariant`, whose detail lists the source map's paths.                                               |
     | `double-panicking`       | Panics.                                                                                                                      |
     | `double-hanging`         | Blocks reading standard input.                                                                                               |
     | `double-flooding`        | Writes beyond the standard-error cap.                                                                                        |
     | `double-exit-failure`    | Writes an outcome, then exits with status 1.                                                                                 |

     _Table 1: The test-double backends and the behaviour each simulates._

     How the doubles reach the expected edits is decided during `EP-M4a`. The
     options are an extra test-only file in the fixture directory, or a
     backend-specific configuration passed by the harness. Either way, record
     the choice in `Decision log`, and confirm that it adds nothing to the
     production contract.
   - `crates/spike_cli/src/registry.rs`: maps backend names to
     `Box<dyn SpikeBackend>`. It registers every double unconditionally,
     because the spike binary is never shipped and a feature-gated registry
     could make the tests silently vacuous.
   - `crates/spike_cli/src/materialize.rs`: implements
     `rewrite-spike materialize --backend <name> --fixture <dir> --outcome
     <file>`. It loads the fixture, restricts the source map to the intent's
     declared files, calls the backend once, and writes the outcome.
   - `crates/spike_harness/src/process.rs`: the bounded process runner from
     `D11`. It takes the path of the executable as a parameter.
   - `crates/spike_harness/src/determinism.rs`: runs `N` fresh processes and
     compares their outcome bytes and exit statuses. It distinguishes
     divergence from `Inconclusive`.
3. Refactor.

Acceptance:

- `determinism.rs` meets its evidence criteria from `Verification plan`.
- The registry-completeness test passes.
- The standard gates pass.

### `EP-M4b` validation pipeline, report, and conformance suite

Assigned: `REQ-PIPELINE`, `REQ-REJECTION`, `REQ-REPORT`, and `REQ-CAPTURE` (as
observed through backends).

1. Red: write `backend_conformance.feature` and its steps, plus unit tests for
   each pipeline stage, using a fake process runner and a fake profile runner.
2. Green: implement, in `crates/spike_harness/src/`:
   - `pipeline.rs`: the 11 steps of ADR 0007 §Validation pipeline, as a flat
     sequence of named stage functions. Each returns a `StageOutcome`, and the
     pipeline stops at the first failed gate.
     - Step 1 uses `fixture::snapshot` only.
     - Expected rejections stop after step 5.
     - Step 4 calls `spike_plan::canonicalize_plan`.
     - The range-set comparison runs against every successful run's plan,
       before step 6.
     - Step 9 runs `cargo fmt --check` on the rewritten copy and records any
       differences as diagnostics only.
     - Step 10 records baseline and candidate outcomes and warning codes for
       each requested profile.
   - `diff.rs`: unified diffs with `similar`, using `a/<path>` and `b/<path>`
     headers and fixture-relative paths only.
   - `report.rs`: `SpikeReport`, serialized with
     `serde_json::to_string_pretty`. It holds:
     - `schema_version`;
     - the backend name, and whether the backend is a test double;
     - the toolchain commit hash;
     - the probe results;
     - the determinism run count;
     - a `corpus_digest` over every manifest and source file;
     - per-fixture results in manifest order; and
     - reserved, empty `dependencies` and `metrics` objects for 14.1.3.

     The report stores error and warning codes, fixture-relative paths, and
     the commit hash. It contains no absolute path, host triple, or time.
     Adding fields keeps the `schema_version`; removing or renaming fields
     bumps it.
   - `conformance.rs`: `run_conformance(executable, backend_name, corpus)`,
     which runs every fixture through the pipeline and returns the report.
3. Add the test-only rejection fixtures under
   `crates/spike_cli/tests/data/`. `own001-stale-snippet` reuses `own001`'s
   sources through the `sources` key, and changes the `binding` capture's
   snippet digest.
4. Wire up
   `rewrite-spike compare --backend <name> --report <path> [--runs <n>]`. An
   unknown backend exits with status 2 and prints
   `no backend named <name> is registered`.
5. Add insta snapshots:
   - the `double-replay` report over all four fixtures; and
   - the unified diff for `own002-owned-param`.

   Pair each snapshot with semantic assertions on the gate outcomes, so a
   snapshot update cannot hide a gate change.
6. Add the end-to-end tests, each run with `--runs 3` against one fixture:
   - `compare` with `double-replay` writes a report in which every gate
     passes;
   - with `double-hash-order`, the report records the divergence; and
   - `compare --backend missing` exits with status 2.

Acceptance:

- The conformance feature passes.
- The snapshots are stable across two consecutive runs.
- The end-to-end tests pass.
- The standard gates pass.

### `EP-M5` documentation, reconciliation, and roadmap closure

1. Amend ADR 0007 as described in `D12`, keeping its status `Proposed`.
   Change its two illustrative `rust,no_run` fences to `rust,ignore`, because
   they refer to types the ADR does not define.
2. Add a short forward note to RFC 0007 §6.2, pointing at ADR 0007's
   "Contract refinements (14.1.1)".
3. Write `spikes/rewrite-materialization/README.md`, covering:
   - the purpose and layout;
   - the package dependency rule;
   - the contract: the trait, the input and output types, the errors, and the
     diagnostic-key table;
   - the `ExpectedSyntax` definitions and extent rules;
   - the validation precedence;
   - the plan semantics and granularity rule;
   - the manifest schema and authoring workflow (including the rule that
     `after/` is never generated);
   - the process protocol;
   - how 14.1.2 adds a backend;
   - the toolchain note for `bor001`;
   - the semantic differences recorded for `bor002`; and
   - the reuse policy.
4. Add a pointer of at most five lines to `docs/developers-guide.md`.
5. Update `docs/repository-layout.md`:
   - add the `spikes/` row between `skills/` and `src/`; and
   - add `verus/rewrite_edit_plan.rs` to the verification assets.
6. `docs/contents.md`: no new document is added under `docs/`, so this file
   does not change.
7. Run this stale-claim search and reconcile every hit with the
   implementation:

   ```bash
   grep -rn "spikes/rewrite-materialization\|SpikeBackend" docs --include=*.md \
     | grep -v docs/execplans
   ```

8. Mark roadmap item 14.1.1 done in `docs/roadmap.md`.
9. Fill in `Outcomes & retrospective`, and set this plan's status to
   `COMPLETE`.

## Milestones and plateaus

### `EP-M0`

- Outcome: the probe's findings are recorded. Nothing is committed except the
  update to this plan.
- Recovery: delete `target/scratch/spike-probe/`.
- Compatibility: none.

### `EP-M1`

- Outcome: the spike workspace exists, is gated locally and in CI, and its
  contract and capture validator are tested.
- Requirements: `REQ-ISOLATION`, `REQ-CONTRACT`, `REQ-CAPTURE`.
- Evidence: the contract tests, the guard tests, the lane contract tests, and
  `cargo tree`.
- Conformance check:
  - capture fields match ADR 0007 §Shared spike interface;
  - no contract type contains a backend type;
  - the dependency rule is in force;
  - the root workspace members are unchanged;
  - no root dependency was added;
  - the `.gitattributes` override is present; and
  - the trace links are current.
- Recovery: revert the milestone commits. The only root files touched are
  `Makefile`, `.gitattributes`, `typos.local.toml`, and the lane contract test.
- Remaining gaps: the kernel, fixtures, and pipeline.
- Compatibility: none.

### `EP-M2`

- Outcome: the edit-plan kernel is implemented, property-tested, bounded
  exhaustively, and proved.
- Requirements: `REQ-HARNESS` (the kernel part).
- Evidence: the properties, the exhaustive test, the Verus output, and the
  seeded-fault transcripts.
- Conformance check:
  - the `D4` semantics are implemented exactly;
  - the Verus model mirrors the forward-cursor shape;
  - the proof contains no `assume`, `admit`, or `external_body`; and
  - the trace links are current.
- Recovery: revert.
- Remaining gaps: the fixtures and the pipeline.
- Compatibility: none.

### `EP-M3`

- Outcome: four consistent fixtures with confirmed compiler outcomes, and the
  `make spike-rewrite-fixtures` command.
- Requirements: `REQ-FIXTURES`.
- Evidence: the command's output and the BDD feature.
- Conformance check:
  - every fixture meets each of its ADR 0007 bullets and RFC 0009
    preconditions;
  - the harness contains no fixture-specific code;
  - `bor001`'s rejection under NLL was observed;
  - the environment scrubbing is covered by the fake-executor tests; and
  - the trace links are current.
- Recovery: revert. A fixture that cannot meet its ADR bullets is an
  architecture escalation.
- Remaining gaps: the process runner, the pipeline, and the report.
- Compatibility: none.

### `EP-M4a`

- Outcome: the process protocol, the test doubles, and the determinism runner
  are proved against real processes.
- Requirements: `REQ-DETERMINISM`.
- Evidence: `determinism.rs`.
- Conformance check:
  - outcomes are written to a file, not to standard output;
  - infrastructure failures are classified as `Inconclusive`;
  - the source map is restricted to the declared files; and
  - the trace links are current.
- Recovery: revert.
- Remaining gaps: the pipeline and the report.
- Compatibility: none.

### `EP-M4b`

- Outcome: the full pipeline, the report, and the conformance suite, all
  proved against the doubles.
- Requirements: `REQ-PIPELINE`, `REQ-REJECTION`, `REQ-REPORT`.
- Evidence: the conformance feature, the snapshots, and the end-to-end tests.
- Conformance check:
  - the 11 pipeline steps map one to one onto stage functions;
  - step 1 reads no captures;
  - rejections stop after step 5;
  - the report contains no absolute path; and
  - the trace links are current.
- Recovery: revert.
- Remaining gaps: the documentation.
- Compatibility: none.

### `EP-M5`

- Outcome: the documentation is reconciled and the roadmap entry is closed.
- Evidence: the Markdown gates and the stale-claim search.
- Compatibility: none.

## Concrete steps

All commands run from the repository root. Each gate runs once, in this order,
with `pipefail` set and its output captured for review:

```bash
set -o pipefail
BR=$(git branch --show-current)
make check-fmt 2>&1 | tee "/tmp/check-fmt-whitaker-${BR}.out"
make lint 2>&1 | tee "/tmp/lint-whitaker-${BR}.out"
make test 2>&1 | tee "/tmp/test-whitaker-${BR}.out"
make test-workflow-contracts 2>&1 | tee "/tmp/workflow-contracts-whitaker-${BR}.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-whitaker-${BR}.out"
make nixie 2>&1 | tee "/tmp/nixie-whitaker-${BR}.out"
```

From `EP-M2` onwards, also run:

```bash
make verus-rewrite-edit-plan 2>&1 | tee "/tmp/verus-whitaker-${BR}.out"
make verus 2>&1 | tee "/tmp/verus-all-whitaker-${BR}.out"
```

From `EP-M3` onwards, also run:

```bash
make spike-rewrite-fixtures 2>&1 | tee "/tmp/spike-fixtures-whitaker-${BR}.out"
```

Focused commands while iterating:

```plaintext
Working directory: repository root
Setup:    SPIKE=spikes/rewrite-materialization/Cargo.toml
          export CARGO_TARGET_DIR="$PWD/target/rewrite-spike"
Red:      cargo nextest run --manifest-path "$SPIKE" -p spike_contract capture
          -> fails: cannot find function `prepare_intent` (EP-M1), then assertion failures until implemented
Green:    same command -> all capture tests pass
Red:      cargo nextest run --manifest-path "$SPIKE" -p spike_plan
          -> fails: cannot find function `canonicalize_plan` (EP-M2)
Green:    same command -> all plan tests, properties, and the exhaustive test pass
Red:      cargo nextest run --manifest-path "$SPIKE" -p spike_cli fixture_corpus
          -> fails: fixture corpus not found (EP-M3)
Green:    same command -> 4 scenarios pass
Red:      cargo nextest run --manifest-path "$SPIKE" -p spike_cli determinism
          -> fails: no backend named double-replay is registered (EP-M4a)
Green:    same command -> replay agrees across 100 runs; hash-order diverges
Red:      cargo nextest run --manifest-path "$SPIKE" -p spike_cli backend_conformance
          -> fails: unknown step "the pipeline runs every primary fixture" (EP-M4b)
Green:    same command -> 8 scenarios pass
```

On the default nextest profile, `make test` skips the installer behaviour
tests. That is unrelated to this item and is not a gate failure.

Commit after each green step that passes the gates. Use an imperative subject
line and a body that explains what changed and why. Documentation-only commits
need only the documentation gates (`make markdownlint`, `make nixie`, and
`make check-fmt`).

## Validation and acceptance

The item is complete when all of the following hold:

- `make spike-rewrite-fixtures` exits zero and prints the outcome table shown
  in `Purpose / big picture`, including `nll=rejected{E0499}` for `bor001`'s
  `after/`.
- `make check-fmt`, `make lint`, and `make test` pass, and their logs show
  the spike targets running.
- `make test-workflow-contracts` passes, including the new
  `SPIKE_REWRITE_TEST=0` assertions.
- `determinism.rs` passes. `double-replay` agrees across 100 processes on
  every primary fixture and on the stale-snippet rejection fixture.
  `double-hash-order` and `double-flaky-rejection` are rejected.
  `double-hanging` and `double-flooding` are reported as `Inconclusive`.
- Every scenario in `backend_conformance.feature` passes.
- `make verus-rewrite-edit-plan` reports zero errors, and the seeded fault was
  rejected before being reverted.
- The `proptest` properties and the exhaustive small-plan test pass, and their
  recorded class counts show every class was reached.
- `cargo tree --manifest-path spikes/rewrite-materialization/Cargo.toml` lists
  no `ast-grep` or `ra_ap` crate, and the root `cargo metadata --no-deps` lists
  no spike package.
- On the pull request, the `coverage-check` lane runs the spike tests, and
  `windows-compat` runs the pure spike packages. Both pass.
- Roadmap item 14.1.1 is marked done.

Quality criteria:

- Tests: every new unit, property, exhaustive, BDD, snapshot, and end-to-end
  test passes.
- Verification: `L-ADJACENT-IMPLIES-PAIRWISE`, `L-FORWARD-EQUALS-SPEC`, and
  `L-OFFSET-MAP`, or their documented refinements, are discharged by Verus.
  Every invariant in `Verification plan` has passing evidence together with its
  non-vacuity control.
- Lint: `make lint` passes. The spike's Clippy run uses the copied root policy
  plus the `disallowed-types` addition.
- Performance: there is no target. Report how much the spike adds to
  `make test` on a quiet host. Investigate if it adds more than five minutes.
- Security: fixture builds run offline, have no dependencies, and run only
  inside the spike's temporary run directories under `target/rewrite-spike/`.

## Idempotence and recovery

Every step can be rerun safely:

- Fixture compiles use fresh temporary directories under
  `target/rewrite-spike/runs/`, and remove them afterwards.
- `rm -rf target/rewrite-spike` removes all spike build output.
- Seeded faults for the non-vacuity checks are made as uncommitted edits,
  recorded, and reverted with `git checkout -- <file>` before the next commit.
- Review any `proptest-regressions/` file a property creates. Keep and commit
  one that records a genuine regression. Delete one created by a seeded fault.

## Artefacts and notes

Planning probes (reconnaissance on 2026-11-16, before approval). They used a
scratch crate under `target/scratch/polonius-probe/`, which is not part of this
plan's deliverables.

```plaintext
rustc 1.98.0-nightly (57d06900f 2026-05-27)
canary [default]          REJECT  error[E0499]: cannot borrow `*a` as mutable more than once at a time
canary [-Zpolonius=off]   REJECT  error: incorrect value `off` for unstable option `polonius`
                                  - either no value or `legacy` (the default), or `next` was expected
canary [-Zpolonius=next]  ACCEPT
bor001_before  [default]          ACCEPT
bor001_before  [-Zpolonius=next]  ACCEPT
bor001_after   [default]          REJECT  error[E0499]: cannot borrow `*map` as mutable more than once at a time
bor001_after   [-Zpolonius=next]  ACCEPT
bor002_after   [default]          ACCEPT
bor002_after   [-Zpolonius=next]  ACCEPT
```

The `bor001` shapes probed are the `before/` and `after/` forms shown in
`EP-M3`. The `bor002_after` probe used the `after/` form described there.

Record here, as they are produced: the `EP-M0` probe transcript, the
seeded-fault transcripts, the Verus output, the property class counts, and the
gate timings.

## Interfaces and dependencies

### Packages

- `crates/spike_contract`, a library.
  - Dependencies: `serde` (with `derive`), `sha2`, and `thiserror`.
  - Dev-dependencies: `rstest`, `googletest`, `pretty_assertions`, and
    `proptest`.
- `crates/spike_plan`, a library.
  - Dependencies: `spike_contract` and `thiserror`.
  - Dev-dependencies: `rstest`, `googletest`, `pretty_assertions`, and
    `proptest`.
- `crates/spike_harness`, a library.
  - Dependencies: `spike_contract`, `spike_plan`, `serde`, `serde_json`,
    `toml`, `sha2`, `thiserror`, `camino`, `cap-std` (with `fs_utf8`),
    `similar`, and `tempfile`.
  - Dev-dependencies: `rstest`, `googletest`, and `pretty_assertions`.
- `crates/spike_test_backends`, a library.
  - Dependencies: `spike_contract`.
- `crates/spike_cli`, the binary `rewrite-spike`.
  - Dependencies: `spike_contract`, `spike_harness`, `spike_test_backends`,
    `serde_json`, and `camino`.
  - Dev-dependencies: `rstest`, `rstest-bdd`, `rstest-bdd-macros`,
    `googletest`, `pretty_assertions`, `insta` (with `json`), and `toml`.

Every requirement is a caret requirement matching the root workspace's version.
Nothing is added to the root workspace. These are the only authorized
dependencies.

### Contract types (`spike_contract`)

The signatures below are the target shape. A field name may change during
implementation only if the change is recorded in `Decision log` and in the
README.

```rust
/// A `/`-separated path relative to the fixture or workspace root.
pub struct WorkspaceRelativePath(String);

/// A SHA-256 digest held as 32 bytes and written as lower-case hexadecimal.
pub struct Sha256Digest([u8; 32]);

/// A half-open byte range `[start, end)` within one source string, serialized as `u64` values.
pub struct ByteRange { pub start: usize, pub end: usize }

/// A capture name: a role, optionally followed by `.` and an index.
pub struct CaptureName { role: String, index: Option<u16> }

pub struct CompilerIdentity { pub rustc_commit_hash: String }

pub enum SemanticNodeIdentity {
    Item { def_path: String, synthetic: bool },
    Expression { owner_def_path: String, syntax_fingerprint: String, synthetic: bool },
}

pub enum ExpansionProvenance { SourceAuthored, EditableMacroCallSite, MacroExpansionOnly }

/// Syntactic units, defined in Rust Reference grammar terms; extents are in the README.
pub enum ExpectedSyntax {
    LetStatement, ExpressionStatement, TailExpression, PathExpression,
    MethodCallExpression, FieldExpression, CallArgument, FunctionParameterType, Expression,
}

pub struct RecipeId { pub kind: RecipeKind, pub version: u16 }

pub enum RecipeKind {
    CloneOnlyUsedByBorrow, OwnedParamCausesClone, LookupThenRelookup, RemoveThenReinsert,
    Unknown(String),
}

pub struct FileIntent { pub path: WorkspaceRelativePath, pub base_file_sha256: Sha256Digest }

pub struct SemanticCapture {
    pub name: CaptureName,
    pub path: WorkspaceRelativePath,
    pub compiler_identity: CompilerIdentity,
    pub node_identity: SemanticNodeIdentity,
    pub expansion: ExpansionProvenance,
    pub base_file_sha256: Sha256Digest,
    pub range: ByteRange,
    pub snippet_sha256: Sha256Digest,
    pub expected_syntax: ExpectedSyntax,
}

pub struct SpikeIntent {
    pub recipe: RecipeId,
    pub files: Vec<FileIntent>,
    pub captures: Vec<SemanticCapture>,
}

pub type SourceMap = BTreeMap<WorkspaceRelativePath, String>;

pub trait SpikeBackend {
    fn name(&self) -> &'static str;
    fn materialize(&self, sources: &SourceMap, intent: &SpikeIntent) -> Result<TextEditPlan, SpikeError>;
}

pub fn prepare_intent<'s>(sources: &'s SourceMap, intent: &'s SpikeIntent) -> Result<PreparedIntent<'s>, SpikeError>;

pub struct TextEdit { pub range: ByteRange, pub replacement: String }
pub struct FileEdits { pub path: WorkspaceRelativePath, pub edits: Vec<TextEdit> }
pub struct TextEditPlan { pub files: Vec<FileEdits> }

pub enum MaterializeOutcome {
    Planned { outcome_schema_version: u16, raw_plan: TextEditPlan },
    Rejected { outcome_schema_version: u16, error: SpikeError, diagnostic: StableDiagnostic },
}
```

`SpikeError` variants:

- `MalformedIntent { fault: IntentFault }`, where `IntentFault` is a closed
  enum with one variant per intent-structure rule;
- `UnsupportedRecipe { kind }`;
- `MacroExpansionOnly { capture }` and `MacroCallSiteNotAllowed { capture }`;
- `MissingSource { capture, path }`;
- `StaleCapture { capture, kind: StaleKind::{File, Snippet} }`;
- `InvalidCaptureRange { capture, fault: RangeFault }`;
- `UnexpectedSyntax { capture, expected }`;
- `AmbiguousSyntax { capture }`;
- `ConflictingCaptures { first, second }`; and
- `UnsupportedRecipeVariant { recipe, detail_key }`.

Backends raise the last four. The contract defines them so that 14.1.2 cannot
invent incompatible ones. Every variant maps to exactly one stable dotted key. A
`StableDiagnostic` holds the key, the capture name, and the path, and nothing
else.

### Plan kernel (`spike_plan`)

```rust
pub fn canonicalize_plan(
    plan: &TextEditPlan,
    declared: &BTreeSet<WorkspaceRelativePath>,
    sources: &SourceMap,
) -> Result<CanonicalPlan, PlanViolation>;
pub fn apply_canonical_plan(plan: &CanonicalPlan, sources: &SourceMap) -> Result<SourceMap, ApplyError>;
pub fn edit_range_set(plan: &CanonicalPlan) -> BTreeMap<WorkspaceRelativePath, Vec<ByteRange>>;
pub fn untouched_bytes_preserved(before: &str, after: &str, edits: &CanonicalFileEdits) -> Result<(), FrameViolation>;
```

`PlanViolation` variants are `OutOfBounds`, `NotCharBoundary`, `NoOpEdit`,
`SharedStart`, `Overlap`, `DuplicateFile`, `EmptyFileEdits`, `UndeclaredFile`,
and `SourceMissing`. Each names the path and the offending edit indices in
canonical order.

### Reuse policy

`spike_contract`, `spike_plan`, `spike_harness`, `spike_test_backends`, and
`spike_cli` exist only for the ADR 0007 spike, and only packages inside
`spikes/rewrite-materialization/` may depend on them. A backend package added
in 14.1.2 may depend on `spike_contract` only. No root-workspace crate may
depend on any spike package.

When 14.2.1 defines the production `whitaker_rewrite_model`, it should lift the
validated parts of the spike by deliberate copy and review, not by depending on
it. Those parts are:

- the `D4` plan semantics;
- the `spike_plan` kernel;
- the Verus proof; and
- the `ExpectedSyntax` extent rules.

At that point it must also revisit Kani for the compiled kernel. ADR 0007's
spike exit artefacts then archive or remove the spike.

## Revision note

- 2026-11-16: initial draft, based on reconnaissance of ADR 0007, RFCs 0007 to
  0009, the repository's gates and CI, earlier ExecPlan budgets, and the pinned
  nightly's borrow-checker behaviour.
- 2026-11-16: revised after a four-member design review; verdict "revise".
  Changes, with their effect on the remaining work:
  - Packages and dependency direction:
    - Split into five packages, removing a dependency cycle and putting the
      kernel out of backends' reach (`D2`).
    - Replaced the feature-gated doubles with an unconditionally registered
      test-backend package.
  - Gate wiring:
    - Replaced the `linux-full` test step, which would have broken the
      lane-deduplication contract, with a `SPIKE_REWRITE_TEST` switch, `ci`
      profile support, and a Windows restriction to the pure packages (`D10`).
  - Fixtures and manifest:
    - Split the fixture snapshot check from the authoring-time corpus check,
      so the harness never preflights captures.
    - Added rejection expectations, test-only rejection fixtures, and
      `ConflictingCaptures` (`D6`).
    - Redesigned fixtures 1 to 4 to match what the detectors emit and what
      RFC 0009 permits (`D13`), and re-probed `bor001` and `bor002`.
  - Contract and plan semantics:
    - Fixed the capture-name grammar.
    - Added closed `IntentFault`, `RecipeKind::Unknown`, per-recipe
      macro-call-site policy, and digest consistency rules.
    - Settled the identity-edit, empty-file-entry, sort-key, and granularity
      rules, and recorded the deviations from RFC 0007 (`D4`).
  - Process runner and compiler runs:
    - Hardened the process protocol (outcome file, concurrent reads, typed cap
      breaches, `Inconclusive` classification) (`D11`).
    - Hardened nested compiler runs (environment scrubbing, toolchain-hash
      check, fresh target directories, artefact evidence, control package).
  - Verification:
    - Made the verification oracles independent: end-to-start application,
      last-edit-first recursion in Verus, and construction-based capture
      precedence.
    - Added the offset-map lemma and the bounded exhaustive test.
  - Scope:
    - Cut `--explain`.
    - Reserved report fields for 14.1.3.
    - Split `EP-M4` into `EP-M4a` and `EP-M4b`, and moved the recipe schemas
      into `EP-M3`.
    - Re-derived the budgets.

[^lsp]: Language Server Protocol Specification 3.17, `TextEdit` and
    `TextDocumentEdit`:
    <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/>.
