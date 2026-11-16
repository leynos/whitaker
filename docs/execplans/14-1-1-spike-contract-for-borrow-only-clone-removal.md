# 14.1.1 Implement the backend-neutral rewrite spike contract and shared fixtures

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (awaiting design review and user approval)

Branch: `14-1-1-spike-contract-for-borrow-only-clone-removal`

Roadmap item: `14.1.1` in `docs/roadmap.md`.

## Purpose / big picture

Whitaker plans to turn some ownership and borrowing lint findings into
compiler-validated source rewrites. A Dylint rule will describe a candidate
rewrite as a `RewriteIntent` with semantic anchors. A materializer will turn
that intent into precise byte-range text edits. A checker will then compile the
rewritten code under the ordinary borrow checker, non-lexical lifetimes (NLL),
and under Polonius, the next-generation borrow checker that accepts some
programs NLL rejects. Architectural decision record (ADR) 0007
(`docs/adrs/adr-0007-rust-rewrite-materialization-substrate.md`) has not yet
chosen the library the materializer will use. It authorizes a comparative
spike: the same four rewrites are implemented once with ast-grep and once with
rust-analyzer's `ra_ap_syntax::SyntaxEditor`, then both run through one shared
harness and fixture corpus, and the evidence picks the winner.

This item builds the part of the spike that does not depend on either library:

- the backend-neutral contract both prototypes must implement (the
  `SpikeBackend` trait, the `SpikeIntent` and `SemanticCapture` input model, the
  `TextEditPlan` output model, and typed errors);
- the shared capture validator that every backend must call before it touches
  syntax;
- the harness that owns everything ADR 0007 assigns to it: fixture integrity,
  edit sorting, overlap detection, edit application, unified diffs, fresh
  backend processes, compiler invocation, and the JSON report; and
- four checked-in primary fixtures, each a small Cargo package with a
  hand-authored capture manifest, the exact expected edit ranges, and the
  expected rewritten source.

Items 14.1.2 and 14.1.3 then add the two real backends, the adversarial corpus,
and the measurements. Nothing in this item adds ast-grep or `ra_ap_syntax` to
any dependency graph.

After this change a maintainer can run, from the repository root:

```plaintext
make spike-rewrite-fixtures
```

and observe that the shared corpus is internally consistent and has the
compiler outcomes ADR 0007 requires. The command prints one line per fixture
and checker profile, and exits zero only when every expectation holds. An
abbreviated example of the expected output:

```plaintext
capability probe: nll rejects canary (E0499); polonius-next accepts canary
own001-clone-only-borrow   before: nll=accept polonius-next=accept  after: nll=accept polonius-next=accept
own002-owned-param         before: nll=accept polonius-next=accept  after: nll=accept polonius-next=accept
bor001-conditional-lookup  before: nll=accept polonius-next=accept  after: nll=reject(E0499) polonius-next=accept
bor002-remove-reinsert     before: nll=accept polonius-next=accept  after: nll=accept polonius-next=accept
4 fixtures consistent; declared edits reproduce every expected output byte-for-byte
```

The test suite also exercises the full ADR 0007 validation pipeline end to end
against test-double backends, including one that is deliberately
nondeterministic and one that widens an edit. This proves the harness rejects
the failures the real backends will later be judged on.

No released Whitaker behaviour changes. The spike lives in a standalone Cargo
workspace that the root workspace does not include, and no shipped crate
depends on it.

## Constraints

- The spike lives under `spikes/rewrite-materialization/` as its own Cargo
  workspace with its own `Cargo.lock`. The root `Cargo.toml` `members` list
  must not include it, and no root workspace package may depend on it (ADR 0007
  §Repository placement).
- No ast-grep crate and no `ra_ap_*` crate may appear in the spike's
  dependency graph in this item.
  `cargo tree --manifest-path spikes/rewrite-materialization/Cargo.toml` must
  not mention `ast-grep` or `ra_ap`.
- The contract package (`spike_contract`) must not depend on the harness
  package, on any filesystem or process API, or on `serde_json`. It holds pure
  domain types and pure validation logic only. The hexagonal dependency rule
  applies: harness and backends depend on the contract, never the reverse.
- Backend-specific node types must not appear in any contract type
  (ADR 0007 §Technical requirements).
- The harness must contain no fixture-specific source-string replacement. All
  fixture knowledge lives in fixture manifests and fixture files (ADR 0007
  §Mandatory gates).
- The harness must not preflight capture digests or capture ranges. Stale and
  invalid capture rejection is part of the backend contract and is observed
  through the backend's own result (ADR 0007 §Shared spike interface).
- Fixture source files must be stored and checked out byte-exact. Git
  end-of-line conversion must be disabled for
  `spikes/rewrite-materialization/fixtures/**`, overriding the repository-wide
  `*.rs text eol=lf` rule in `.gitattributes`.
- Contract and harness output must be deterministic: no `HashMap`, `HashSet`,
  random state, wall-clock time, process identifiers, or absolute paths in any
  serialized plan, error, diagnostic, or report.
- Repository conventions from `AGENTS.md` apply to all spike code: files of at
  most 400 lines, a `//!` module comment on every module, `///` comments on
  every function, no `unwrap` or `expect` outside tests, no `unsafe`,
  item-scoped `#[expect(..., reason = "...")]` only as a last resort, assertion
  messages on every `assert!`, caret version requirements, en-GB-oxendict
  spelling, and `cap_std`/`camino` for filesystem access.
- Production spike code must not read environment variables except at the
  `rewrite-spike` binary's composition root. Subprocess environments are
  configured with `Command::env` and `Command::env_remove`, which confine the
  change to the child process.
- Compiler admission must not be bypassed. Fixture builds keep the caller's
  `RUSTC_WRAPPER`, use the repository's pinned toolchain, and use a target
  directory under `spikes/rewrite-materialization/target/`, never `/tmp`.
- `docs/users-guide.md` does not change: nothing user-visible ships.
- Completed ExecPlans under `docs/execplans/` must not be edited.
- Implementation begins only after the user explicitly approves this plan.

## Tolerances (exception triggers)

Every size count below excludes this ExecPlan file. The comparison base is the
merge base with `origin/main` (`40a85aa5` at drafting time). Counts are net
lines (additions minus deletions) measured with:

```bash
git diff --numstat "$(git merge-base origin/main HEAD)" -- \
  ':!docs/execplans/14-1-1-spike-contract-for-borrow-only-clone-removal.md'
```

Classification is by whole file:

- Code: `*.rs` files outside `fixtures/`, the Verus proof, `Makefile` and
  `scripts/*.sh` hunks, and the BDD `*.feature` files.
- Fixture data: everything under `spikes/rewrite-materialization/fixtures/`
  (Rust sources, fixture `Cargo.toml` files, and manifests) and insta snapshot
  files. These are tracked separately because a fixture's size follows from the
  ADR's fixture requirements, not from implementation effort.
- Configuration: the spike's `Cargo.toml` files, `clippy.toml`,
  `.config/nextest.toml`, `rust-toolchain.toml` if added, `.gitattributes`,
  `typos.local.toml`, and `.github/workflows/*.yml`. The spike `Cargo.lock` is
  generated and excluded from the line count, but its addition is mandatory and
  must be committed.
- Documentation: Markdown outside `docs/execplans/`.

Budget survey. No comparable plan creates a standalone workspace. The nearest
analogues are `docs/execplans/7-1-1-whitaker-sarif-crate.md` (one new crate: 20
files, 2,000 net lines, finished at the file cap) and
`docs/execplans/2-4-1-string-continuation-style-post-expansion-early-lint.md`
(two new crates: 30 files and 1,800 net lines, exceeded to 51 paths and 3,507
lines and re-authorized by the owner; the cause was approved milestone growth
plus pre-existing branch work, not missed scope).
`docs/execplans/6-1-1-shared-lcom4-helper.md` finished exactly at its 400-line
cap. No plan in the history kept per-category budgets, so there is no
category-level precedent. The lesson taken is that new-crate plans land at or
over a single combined cap. This plan therefore sets explicit per-category
budgets sized from the work breakdown in `Plan of work`, with roughly 25%
contingency on code.

The estimate behind the code budget is: contract package about 900 lines of
production code and 700 of tests; harness package about 1,500 production and
1,300 tests, including BDD steps and end-to-end tests; test-double backends
about 250; Verus model about 300; Makefile, script, and runner wiring about 60.
That totals about 5,000.

- Files: if more than 110 non-ExecPlan files change, stop and escalate.
- Code: if code exceeds 6,000 net lines, stop and escalate.
- Fixture data: if fixture data exceeds 1,500 net lines, stop and escalate.
- Configuration: if configuration exceeds 400 net lines, stop and escalate.
  This budget includes a copy of the root lint tables into the spike manifest,
  about 90 lines.
- Documentation: if documentation exceeds 700 net lines, stop and escalate.
- Single file: if any Rust file would exceed 400 lines, split it before
  committing. This is a repository rule, not a tolerance.
- Interface: no root-workspace public API may change. Any need to change one
  is an escalation.
- Dependencies: only the dependencies listed in `Interfaces and dependencies`
  are authorized. Any other crate, any ast-grep or `ra_ap_*` crate, or any new
  root-workspace dependency is an escalation.
- Architecture: if the contract cannot express one of the four primary
  fixtures without a backend-specific type, or if the harness needs
  fixture-specific code, stop and escalate. Either would falsify ADR 0007's
  shared-interface assumption.
- Toolchain: if the pinned nightly stops accepting `-Zpolonius=next`, or the
  `bor001` expected output stops being NLL-rejected and Polonius-accepted, stop
  and escalate. Do not change the toolchain pin in this item.
- CI: if wiring the spike into CI requires changing the workflow-contract tests
  under `tests/workflow_contracts/` beyond adding expected entries, stop and
  escalate.
- Verus: if a Verus obligation is not discharged after one working day, or the
  proof run exceeds 10 minutes, stop and escalate. Do not weaken the theorem or
  add `assume`, `admit`, or `external_body` to make it pass.
- Iterations: if a gate still fails after three fix attempts for the same
  cause, stop and escalate.
- Time: if any one milestone takes more than two working days, stop and
  escalate.
- Ambiguity: if ADR 0007 and RFCs 0007 to 0009 conflict in a way this plan has
  not already resolved in `Decision log`, stop and present options.

Each category budget is enforced independently. Spare capacity in one category
does not offset an overrun in another.

## Risks

- Risk: the standalone workspace inherits root configuration in surprising
  ways. Rustfmt, Cargo configuration, and `rust-toolchain.toml` are discovered
  by searching parent directories; Clippy's `clippy.toml` and nextest
  configuration may or may not be. Severity: medium. Likelihood: medium.
  Mitigation: the `EP-M0` probe records which root files the spike inherits
  before any code is written, and the spike carries its own `clippy.toml` and
  `.config/nextest.toml` deliberately.

- Risk: fixture packages copied under the spike's `target/` directory are
  treated as members of the spike workspace, and Cargo refuses to build them.
  Severity: medium. Likelihood: high without mitigation. Mitigation: every
  fixture `Cargo.toml` declares an empty `[workspace]` table, which makes each
  copy its own workspace root. `EP-M0` confirms this.

- Risk: Linux CI never runs `make test`. The `linux-full` job runs `coverage`,
  and only `windows-compat` runs `make test NEXTEST_PROFILE=ci`. Wiring the
  spike into `make test` alone would test it only on Windows. Severity: high.
  Likelihood: certain. Mitigation: `EP-M1` adds a `make spike-rewrite-check`
  step to `linux-full`, and `make test` also runs it so the local gate covers
  it. `tests/workflow_contracts/` lane tests may need an entry. That is a
  tolerance trigger if it is more than an entry.

- Risk: the Windows lane runs the spike through `make test`, and the fixture
  compiler runs or byte-fidelity checks behave differently there. Severity:
  medium. Likelihood: low to medium. Mitigation: `WorkspaceRelativePath` always
  uses `/` separators, fixtures are stored with Git text conversion disabled,
  and every subprocess path is built with `camino`. If Windows still fails for
  host reasons, record it in `Surprises & discoveries` and escalate rather than
  skipping the spike silently on one platform.

- Risk: the Polonius behaviour that fixture `bor001` depends on changes when the
  toolchain pin moves. Severity: medium. Likelihood: low for this item.
  Mitigation: the harness runs a capability probe before every profile run and
  reports `unavailable` rather than a misleading result. The fixture README
  records that the expectation must be reviewed on every toolchain bump (ADR
  0007 §Known risks).

- Risk: hand-authored byte offsets in fixture manifests are easy to get wrong.
  Severity: medium. Likelihood: high. Mitigation: each capture and each
  expected edit also records its literal text. The fixture integrity check
  verifies range, digest, and literal text together, and verifies that applying
  the declared edits to `before/` reproduces `after/` exactly. A
  `rewrite-spike fixtures --explain` mode prints each range's bytes to help
  authors.

- Risk: the shared interface accidentally favours one backend (ADR 0007
  §Known risks). Severity: medium. Likelihood: medium. Mitigation: the contract
  carries only byte ranges, digests, opaque identities, and a closed
  `ExpectedSyntax` vocabulary. Nothing in it presumes a template language or a
  typed syntax tree. Review checks that no contract type was shaped by either
  library's API.

- Risk: 100 fresh backend processes per fixture make the test suite slow.
  Severity: low. Likelihood: low. Mitigation: the process runner is a small
  binary that reads one fixture and one intent. The full 100-run gate runs in
  one end-to-end test per double. Unit tests drive the comparison logic with an
  injected run count.

- Risk: nested Cargo builds wait behind the shared compile-admission queue and
  hit nextest timeouts. Severity: medium. Likelihood: medium. Mitigation: the
  spike's `.config/nextest.toml` gives compiler-backed tests a generous
  `slow-timeout` and places them in a test group with bounded concurrency. A
  timeout whose only output is Cargo's package-cache lock message is treated as
  queue starvation, not a candidate failure.

## Progress

- [x] (2026-11-16 12:10Z) Branch renamed and session titled.
- [x] (2026-11-16 12:20Z) Reconnaissance: ADR 0007; RFCs 0007, 0008, and 0009;
  repository gates and CI; past plan budgets; design and testing guides.
- [x] (2026-11-16 12:30Z) Planning probe: the pinned nightly's NLL and Polonius
  behaviour recorded (see `Artefacts and notes`).
- [x] (2026-11-16 13:30Z) Initial draft written.
- [ ] Design review by a community-of-experts panel; revisions applied.
- [ ] Plan approved by the user.
- [ ] `EP-M0` workspace and inheritance probe.
- [ ] `EP-M1` spike workspace, contract model, and capture validation.
- [ ] `EP-M2` edit-plan kernel and Verus proofs.
- [ ] `EP-M3` primary fixtures, fixture integrity, and checker profiles.
- [ ] `EP-M4` backend pipeline, determinism runner, report, and conformance
  kit.
- [ ] `EP-M5` documentation, reconciliation, and roadmap closure.

## Surprises & discoveries

- Observation: on the pinned toolchain `nightly-2026-05-28`
  (`rustc 1.98.0-nightly (57d06900f 2026-05-27)`), the default borrow checker
  is NLL. `-Zpolonius=next` selects Polonius Alpha, and `-Zpolonius=off` is
  rejected as an invalid value. Evidence: the planning probe in
  `Artefacts and notes`. Impact: the NLL profile on this toolchain passes no
  `-Zpolonius` flag at all (`D5`). A later toolchain that makes Polonius the
  default will need `-Zpolonius=off` for NLL, which is why the profile runner
  selects flags from a probed capability rather than hard-coding them.

- Observation: the repository's `.gitattributes` forces `*.rs text eol=lf`.
  Evidence: `.gitattributes` lines 1 to 9. Impact: without an override, a CRLF
  fixture (needed by 14.1.2) would be normalized on checkout and every digest
  would silently change. `EP-M1` adds a `-text` override for the fixture tree.

- Observation: the RFCs leave several plan semantics open. RFC 0007 §10.2
  generates "sorted and unsorted" inputs without saying whether unsorted input
  is rejected or normalized, and no RFC addresses zero-width insertions or
  edits that share a start offset. Evidence: RFC 0007 lines 285 to 296 and 1044
  to 1049. Impact: `D4` fixes these semantics for the spike, and `EP-M5`
  records them in ADR 0007.

## Decision log

- D1. Decision: scope. This item delivers the contract, the shared capture
  validator, the edit-plan kernel, the harness pipeline, the four primary
  fixtures, a backend conformance kit, and test-double backends. It does not
  deliver either real backend, the adversarial fixture corpus, timing or
  footprint measurements, the spike report, or the ADR outcome. The contract
  does define every typed error and harness check the adversarial corpus will
  need, because 14.1.2 must not change the contract to add fixtures. Rationale:
  the roadmap splits the spike into contract and fixtures (14.1.1), backends
  and adversarial fixtures (14.1.2), and evidence and selection (14.1.3). A
  harness built without both real backends can only be proved against doubles,
  so the doubles are part of this item's evidence, not throwaway scaffolding.
  Date/Author: 2026-11-16, planning agent.

- D2. Decision: the spike workspace has two packages now,
  `crates/spike_contract` and `crates/spike_harness`, and 14.1.2 adds one
  package per backend. Each backend package depends on `spike_contract` only,
  and the `rewrite-spike` binary in `spike_harness` depends on the backends.
  Rationale: ADR 0007 §Migration plan requires each backend to be implemented
  "without importing" the other library. Separate packages make Cargo enforce
  that rule, whereas modules in one crate would need review to catch it. The
  ADR's file tree shows a single `src/` directory, but that tree is
  illustrative ("should live"). This plan keeps its top-level items:
  `Cargo.toml`, `Cargo.lock`, `README.md`, `fixtures/`, and the harness and
  backend roles. The rejected alternative is a single crate with Cargo features
  per backend: features unify across a workspace build, so they cannot enforce
  the separation. Date/Author: 2026-11-16, planning agent.

- D3. Decision: capture validation is a contract function,
  `spike_contract::prepare_intent`, that every backend calls first. It returns a
  `PreparedIntent` whose `ValidatedCapture` values can only be built by that
  function, so a backend cannot reach a capture's text without passing
  validation. The harness never calls it before invoking a backend. Rationale:
  ADR 0007 requires each materializer to reject stale and invalid captures, and
  requires that the harness not preflight them, so that both backends face the
  same rejection contract. A shared validator gives byte-identical rejection
  behaviour across backends and keeps validation out of the backend line counts
  that 14.1.3 compares. The conformance kit still observes rejection only
  through `SpikeBackend::materialize`, so a backend that skips the call fails
  the kit. The rejected alternative is for each backend to write its own
  checks, which would duplicate logic the ADR wants identical and would charge
  one backend for code the other might write more tersely. Date/Author:
  2026-11-16, planning agent.

- D4. Decision: edit-plan semantics for the spike. A `ByteRange` is half-open,
  `[start, end)`, in bytes of the original file. The harness, not the backend,
  sorts each file's edits by `(start, end)` and files by path. A canonical file
  edit list is well formed when every range lies within the file, both offsets
  are UTF-8 character boundaries, no edit is a no-op (an empty range with an
  empty replacement), starts are strictly increasing, and each edit's `end` is
  at most the next edit's `start`. Two edits that share a start offset
  conflict, even when one is a zero-width insertion, because their output order
  would otherwise depend on emission order. Edits that touch end to start are
  allowed. A plan may name each file at most once and may edit only files
  declared in the intent's `files`. Rationale: the Language Server Protocol
  (LSP) defines edit ranges against the original document, forbids overlap, and
  orders same-position insertions by array position.[^lsp] Ordering by array
  position conflicts with the requirement that the harness own sorting, so the
  spike rejects the case instead. RFC 0007 §6.2 already requires non-overlap,
  bounds, UTF-8 boundaries, and unique paths. This decision adds the same-start
  and no-op rules and settles that unsorted input is normalized, not rejected.
  Date/Author: 2026-11-16, planning agent.

- D5. Decision: checker profiles. The harness supports two profiles, `nll` and
  `polonius-next` (RFC 0007's `BorrowCheckerProfile` wire names). Before any
  fixture run, it compiles a two-line canary under each candidate flag set. The
  `nll` profile uses the first of "no flag" and `-Zpolonius=off` that the
  compiler accepts and that rejects the canary. The `polonius-next` profile uses
  `-Zpolonius=next` and must accept the canary. If no flag set matches, that
  profile is reported as `unavailable` and every fixture expectation under it
  fails. On the pinned nightly this selects "no flag" for `nll`. Rationale: the
  planning probe showed that this nightly rejects `-Zpolonius=off`, while
  current nightlies default to Polonius and need it for NLL. RFC 0007
  §Capability probing already requires a probe, because finding the flag in
  `rustc -Z help` does not prove it works. Date/Author: 2026-11-16, planning
  agent.

- D6. Decision: fixture representation. Each fixture is a directory
  `fixtures/<fixture-id>/` with a `manifest.toml`, a `before/` Cargo package,
  and an `after/` Cargo package. Each package's `Cargo.toml` declares an empty
  `[workspace]` table and no dependencies, and uses edition 2024. The manifest
  holds the intent, captures, expected edits, expected compiler outcomes, and
  recipe metadata. Each capture and each expected edit records its literal text
  as well as its offsets and digests. ADR 0007's top-level `expected/`
  directory becomes each fixture's `after/` directory, so a fixture's input and
  expected output sit together. Rationale: compiler validation needs real Cargo
  packages, and the repository's "group by feature" rule favours colocation.
  The literal text makes hand-authored offsets reviewable. Date/Author:
  2026-11-16, planning agent.

- D7. Decision: synthetic semantic identity. Fixtures are hand-authored, so
  their `compiler_identity` and `node_identity` values cannot come from a real
  compiler run. The manifest records the pinned compiler's `rustc -vV` commit
  hash as `compiler_identity`, and records each `node_identity` as a structured
  value (an item path for items, or an owner path plus a syntax fingerprint for
  expressions) whose strings are marked `synthetic` in the manifest schema.
  Backends must treat identities as opaque and may only compare them for
  equality. Rationale: ADR 0007 requires fixtures with hand-authored captures
  "equivalent to those a Dylint rule would emit", so that the spike measures
  materialization rather than discovery. Marking the values as synthetic stops
  anyone from mistaking them for compiler evidence. Date/Author: 2026-11-16,
  planning agent.

- D8. Decision: recipe identifiers use RFC 0009's stable dotted kinds with an
  integer version: `ownership.clone_only_used_by_borrow`,
  `ownership.owned_param_causes_clone`, and `borrow.lookup_then_relookup`. The
  fourth recipe uses `borrow.remove_then_reinsert`, which RFC 0009 §8.3 does
  not yet list. Fixture 4 carries `confidence = "review_required"`, which the
  report must propagate unchanged. Rationale: these names keep the spike
  aligned with the production intent model. The missing kind is recorded as a
  gap for RFC 0009 rather than invented silently. Date/Author: 2026-11-16,
  planning agent.

- D9. Decision: verification mix. Verus proves two edit-plan theorems over an
  independent sequence model: adjacent-pair checking implies pairwise
  disjointness, and forward cursor-based application equals the
  segment-interleaving specification. `proptest` checks the production code
  against independent oracles. `rstest` tables cover the finite capture
  validation precedence and error classes. `rstest-bdd` covers the observable
  harness behaviour. Kani is not used. See `Formal verification decision`.
  Date/Author: 2026-11-16, planning agent.

- D10. Decision: gate wiring. New Makefile targets `spike-rewrite-fmt`,
  `spike-rewrite-check-fmt`, `spike-rewrite-lint`, `spike-rewrite-test`,
  `spike-rewrite-check` (all three checks), and `spike-rewrite-fixtures` drive
  the spike with `--manifest-path`. Root `fmt`, `check-fmt`, `lint`, and `test`
  invoke the matching spike target, so the commands the user requires
  (`make check-fmt`, `make lint`, `make test`) cover the spike. `linux-full` in
  `.github/workflows/ci.yml` gains a `make spike-rewrite-test` step, because
  Linux CI runs `coverage` rather than `make test`. The spike is not added to
  `make coverage` or to the Whitaker Dylint run. Rationale: ADR 0007 allows a
  repository target to run the spike, and a standalone workspace is otherwise
  invisible to every gate. Date/Author: 2026-11-16, planning agent.

- D11. Decision: determinism and resource bounds. Spike code uses `BTreeMap`
  and `Vec` only. The spike's `clippy.toml` copies the root settings and adds
  `disallowed-types` for `std::collections::HashMap` and `HashSet`, so
  accidental hash ordering fails `make lint`. The backend process runner caps
  each child's captured standard output at 1 MiB and standard error at 64 KiB,
  and stops a child after 60 seconds, reporting a typed failure. Fixture
  compiler runs have a 15-minute deadline, because they wait in the shared
  compile-admission queue. Date/Author: 2026-11-16, planning agent.

- D12. Decision: documentation placement. ADR 0007 is still `Proposed`, so the
  contract refinements from `D3` to `D8` are written into its §Shared spike
  interface and §Outstanding decisions in place, rather than as a new ADR.
  `spikes/rewrite-materialization/README.md` documents the internal interfaces
  as the spike's component architecture document. `docs/developers-guide.md`
  gains a section on running and extending the spike, including its reuse
  policy. `docs/repository-layout.md` gains the `spikes/` entry.
  `docs/users-guide.md` does not change. Date/Author: 2026-11-16, planning
  agent.

## Outcomes & retrospective

Not started. Record outcomes at each milestone boundary.

## Context and orientation

Inspected base revision: `40a85aa5` (`origin/main`, "Score desugared loops and
`?` as one authored construct (#623)").

### The rewrite pipeline and where this item sits

Three proposed RFCs describe the pipeline. RFC 0007
(`docs/rfcs/rfc-0007-compiler-validated-rewrite-checking.md`) defines the
checker that replays a candidate under NLL and Polonius. RFC 0008
(`docs/rfcs/rfc-0008-borrow-workaround-lint-family.md`) defines the
borrow-workaround lint family, whose codes `BOR001` (`lookup_then_relookup`) and
`BOR002` (`remove_then_reinsert`) motivate fixtures 3 and 4. RFC 0009
(`docs/rfcs/rfc-0009-validated-ownership-and-borrow-rewriter.md`) defines the
rewriter and its materializers, including `clone_only_used_by_borrow` and
`owned_param_causes_clone`, which motivate fixtures 1 and 2. All three are
`Proposed`. ADR 0007 sits between them and the code: it decides how edits are
materialized, and its §Comparative spike is the normative specification for
this item. Read ADR 0007 §Comparative spike in full before starting.

The vocabulary used by the RFCs that this plan aligns with:

- RFC 0007 §6.2 defines
  `TextEdit { start_byte: u64, end_byte: u64, replacement: String }` and
  `FileEditSet { path, base_sha256, edits }`, and requires edits to be
  non-overlapping, in bounds, on UTF-8 boundaries, and applied end to start.
- RFC 0009 §7.1 defines `SemanticAnchor` with `base_file_sha256`, a byte
  `span`, and `snippet_sha256`.
- RFC 0007 names the checker profiles
  `BorrowCheckerProfile { Nll, PoloniusNext }`, written `nll` and
  `polonius-next` on the command line.
- RFC 0008 and RFC 0009 define
  `SemanticConfidence { Mechanical, LocallyReasoned, ReviewRequired }`.

ADR 0007 references several types it never defines: `ByteRange`, `TextEditPlan`,
`SpikeError`, `RecipeId`, `FileIntent`, `CompilerIdentity`,
`SemanticNodeIdentity`, `ExpansionProvenance`, `Sha256Digest`,
`ExpectedSyntax`, and `WorkspaceRelativePath`. This item defines all of them in
`spike_contract` (see `Interfaces and dependencies`). The spike's names are
deliberately close to the RFCs' names, but the spike types are not the
production model. Roadmap item 14.2.1 defines the versioned production
`RewriteIntent` and `TextEditPlan` in `whitaker_rewrite_model`, informed by the
spike.

### Terms used in this plan

- Capture: one `SemanticCapture`, a named, compiler-anchored byte range in one
  source file, with digests of the whole file and of the range's bytes.
- Stale capture: a capture whose file digest or snippet digest no longer
  matches the supplied source.
- Materialize: turn an intent and its source files into a `TextEditPlan`
  without writing anything.
- Raw plan: the exact bytes a backend process writes for its plan, before the
  harness sorts or validates anything.
- Canonical plan: the harness's sorted and validated form of a raw plan.
- Checker profile: one borrow-checker configuration, `nll` or
  `polonius-next`.
- Test double: a backend implementation used only by tests, either
  well behaved or deliberately faulty.
- Conformance kit: a set of checks, generic over any `SpikeBackend`, that a
  backend must pass. 14.1.2 runs it against both real backends.

### Repository facts that shape the plan

The root workspace (`Cargo.toml` line 2) lists `cli`, `common`, `crates/*`,
`installer`, `suite`, and `vendor/dylint_testing`, and has no `exclude`. No
standalone Cargo workspace exists anywhere in the repository today. A package
nested under the root without its own `[workspace]` table is rejected by Cargo,
which `Makefile` lines 123 to 126 already note for scratch trees.

The root lint policy lives in `[workspace.lints.clippy]`,
`[workspace.lints.rust]`, and `[workspace.lints.rustdoc]` (`Cargo.toml` lines
145 to 222). It denies, among others, `unwrap_used`, `expect_used`,
`indexing_slicing`, `string_slice`, `missing_docs`, `missing_const_for_fn`,
`must_use_candidate`, `shadow_reuse`, and `cognitive_complexity`, and forbids
`unsafe_code`. A separate workspace inherits none of this, so the spike must
copy it.

Root `.rustfmt.toml` enables unstable features (`imports_granularity`,
`group_imports`, `wrap_comments`, and `fn_single_line`). Rustfmt discovers it
by searching parent directories, so spike sources should pick it up. `EP-M0`
verifies this. Root `.cargo/config.toml` sets
`[unstable] no-embed-metadata = true`, which Cargo also discovers from parent
directories. `rust-toolchain.toml` pins `nightly-2026-05-28`. Rustup discovers
it from parent directories, so the spike and every fixture copy under its
`target/` use the same compiler.

Gate targets (Makefile, line numbers at `40a85aa5`): `fmt` (582), `check-fmt`
(587), `lint` (567; it runs `interrogate`, `skill-manifest-check`,
`lint-clippy`, and `lint-whitaker`), `lint-clippy` (569), `test` (283),
`markdownlint` (591, which depends on `spelling`), `spelling` (594), `nixie`
(597), and `verus` (648). Locate them with `grep -n '^check-fmt:' Makefile`.
`cargo fmt --all` formats only workspace members, so the root target never
reaches the spike. `mdtablefix --git` and `markdownlint` do scan Markdown under
`spikes/`, so the spike README must pass the root Markdown policy.

CI (`.github/workflows/ci.yml`): `linux-full` runs `make check-fmt` (line 492),
`make lint` (589), `spelling`, `nixie`, and Markdown lint, and the
`coverage-check` job runs coverage. Only `windows-compat` runs
`make test NEXTEST_PROFILE=ci` (line 818). `.github/workflows/proofs.yml` runs
`make verus` and `make kani`, filtered by a `paths` list (lines 26 to 39) that
includes `verus/**` and `scripts/**`.

Verus proofs are standalone model files in `verus/`, one per property, listed
by name in `scripts/run-verus.sh` groups and run through `make verus` or a
named target. The pinned Verus release is `tools/verus/VERSION`
(`0.2026.03.17.a96bad0`). The closest precedent is
`verus/string_continuation_rewrite.rs`, which proves that removing sorted,
disjoint byte ranges equals concatenating the surrounding fragments. Its
`ByteRange` model and recursive "last range first" style are the template for
this item's proof. Kani harnesses live under `#[cfg(kani)]` inside production
crates and are run by `scripts/run-kani.sh`.

Locked versions in the root `Cargo.lock` that the spike should match, to reuse
the shared build cache: `rstest` 0.28.0, `rstest-bdd` 0.6.0, `googletest`
0.14.4, `pretty_assertions` 1.4.1, `insta` 1.49.0, `proptest` 1.12.0, `sha2`
0.11.1, `serde_json` 1.0.154, `cap-std` 4.0.3, `camino` 1.2.6, `similar` 2.7.0
(transitive in the root), `tempfile` 3.27.0, and `thiserror` 2.

### Testing references

- `docs/rust-testing-with-rstest-fixtures.md`: fixtures, `#[case]` tables,
  `#[from]` renaming. Its version numbers are stale (it says 0.18); use
  `rstest` 0.28 as locked.
- `docs/rstest-bdd-users-guide.md`: feature files under `tests/features/`,
  `#[scenario(path = ..., name = ...)]` or `scenarios!`, `{name:Type}`
  placeholders, and steps that return `Result`. Its examples say 0.2; check the
  0.6 API against existing repository tests such as those under `cli/tests/`.
- `docs/developers-guide.md` lines 290 to 315: `panic_in_result_fn` is denied,
  so assertions belong in `()`-returning `then` steps and `given`/`when` steps
  return `Err` rather than asserting.
- `docs/rust-doctest-dry-guide.md`: doctests compile as separate crates;
  fallible examples use `fn main() -> Result<...>`; complex or private examples
  belong in unit tests.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`: avoid the
  bumpy-road shape. Keep the pipeline a flat sequence of named stage functions
  rather than one function with nested blocks per stage.

### Design references

- `docs/whitaker-dylint-suite-design.md`: the suite design. It has no rewrite
  section, so this item does not change it.
- `docs/whitaker-clone-detector-design.md` lines 240 to 252 and 518 to 522: the
  existing byte-span discipline (half-open, in bounds, UTF-8 aligned, typed
  errors instead of panics). Lines 667 to 674 show the precedent of confining a
  syntax library to one adapter file.
- `docs/ownership-shape-lints-design.md` §12 to §14: the Phase 9 ownership
  lints that fixtures 1 and 2 imitate.
- `docs/adrs/adr-0003-formal-proof-strategy-for-the-clone-detector-pipeline.md`:
  Verus for small algebraic invariants, Kani for bounded execution, tests for
  broad behaviour.

### Skills to load during implementation

Load `rust-router` first, then: `rust-types-and-apis` for the contract surface
and newtypes; `rust-errors` for `SpikeError` and harness errors;
`rust-unit-testing` for `rstest`, `googletest`, `pretty_assertions`, and
`insta`; `proptest` for the oracle properties; `verus` for `EP-M2`;
`hexagonal-architecture` to keep the contract free of adapters;
`domain-cli-and-daemons` for the `rewrite-spike` binary and subprocess handling;
`nextest` for the spike's nextest configuration; and `nll-to-polonius` if the
`bor001` fixture needs revisiting.

## Conformance basis

Upstream artefacts, all inspected at `40a85aa5`:

- ADR 0007, `docs/adrs/adr-0007-rust-rewrite-materialization-substrate.md`
  (Status: Proposed, dated 2026-08-30), especially §Technical requirements,
  §Repository placement, §Shared spike interface, §Primary rewrite fixtures,
  §Adversarial fixtures, §Validation pipeline, §Mandatory gates, and §Migration
  plan steps 1 and 2.
- RFC 0007 §6.2 (edit model and invariants), §Capability probing, and §10.2.
- RFC 0008 §5.2 to §5.3 (intent and anchor model) and the BOR table.
- RFC 0009 §7.1 to §7.3 (anchors, staleness, macros) and §8.3 (recipe kinds).
- `docs/roadmap.md` item 14.1.1.
- `AGENTS.md` (code, testing, and documentation rules) and
  `docs/documentation-style-guide.md`.

No terms-of-reference document or separate technical design exists for the
spike. ADR 0007 plays the technical-design role.

Requirements derived from those artefacts:

- `REQ-ISOLATION`: the spike is a standalone workspace that is not a root
  member and adds neither candidate library (ADR 0007 §Repository placement).
- `REQ-CONTRACT`: one backend-neutral `SpikeBackend` trait and edit model
  that both backends implement, with no backend types (ADR 0007 §Shared spike
  interface and §Technical requirements).
- `REQ-CAPTURE`: captures carry compiler identity, node identity, expansion
  provenance, file digest, half-open range, snippet digest, and expected
  syntax. Macro-expansion-only captures are rejected before any edit. Range
  validity is checked before slicing or hashing. Stale digests are rejected by
  the materializer with typed errors (ADR 0007 §Shared spike interface).
- `REQ-HARNESS`: the harness owns fixture integrity, sorting, overlap
  detection, application, diffs, compiler invocation, and report serialization,
  and contains no fixture-specific replacement (ADR 0007 §Shared spike
  interface and §Mandatory gates).
- `REQ-FIXTURES`: four primary fixtures with exact expected per-file edit
  ranges, expected outputs, and the compiler outcomes ADR 0007 states for each
  (ADR 0007 §Primary rewrite fixtures).
- `REQ-PIPELINE`: the 11-step validation pipeline, including fresh backend
  processes, exact range-set comparison, untouched-byte checks, formatting
  diagnostics, and baseline and candidate profile outcomes (ADR 0007
  §Validation pipeline).
- `REQ-DETERMINISM`: exactly 100 fresh backend processes per fixture, with
  byte-identical raw plans for successes and identical typed errors and stable
  diagnostic data for rejections (ADR 0007 §Validation pipeline and §Mandatory
  gates).
- `REQ-REPORT`: a stable, machine-readable JSON report (ADR 0007
  §Validation pipeline step 11 and §Spike exit artefacts).

Gaps between the baseline and the target:

- `GAP-TYPES`: ADR 0007 names types it never defines.
- `GAP-PLAN-SEMANTICS`: the RFCs leave sorting, same-start, and no-op edit
  semantics open.
- `GAP-NLL-FLAG`: the pinned nightly rejects `-Zpolonius=off`.
- `GAP-GATES`: a standalone workspace is invisible to every gate, and Linux CI
  does not run `make test`.
- `GAP-EOL`: `.gitattributes` would normalize fixture line endings.
- `GAP-RECIPE-KIND`: RFC 0009 lists no kind for `remove_then_reinsert`.

Trace links:

```plaintext
ADR0007 §Repository placement -> REQ-ISOLATION -> D2, D10 -> EP-M0, EP-M1
    -> make spike-rewrite-check; cargo tree check
ADR0007 §Shared spike interface -> REQ-CONTRACT, GAP-TYPES -> D2, D7, D8 -> EP-M1
    -> spike_contract unit tests
ADR0007 §Shared spike interface -> REQ-CAPTURE -> D3 -> EP-M1
    -> capture tests; conformance kit stale and invalid-range checks (EP-M4)
RFC0007 §6.2 -> REQ-HARNESS, GAP-PLAN-SEMANTICS -> D4, D9 -> EP-M2
    -> plan properties; verus/rewrite_edit_plan.rs
ADR0007 §Primary rewrite fixtures -> REQ-FIXTURES, GAP-NLL-FLAG, GAP-EOL -> D5, D6 -> EP-M3
    -> make spike-rewrite-fixtures; fixture_corpus.feature
ADR0007 §Validation pipeline -> REQ-PIPELINE, REQ-DETERMINISM, REQ-REPORT -> D11 -> EP-M4
    -> backend_conformance.feature; report snapshots
GAP-GATES -> D10 -> EP-M1 -> make check-fmt, make lint, and make test include the spike; CI step
GAP-RECIPE-KIND, GAP-PLAN-SEMANTICS, GAP-NLL-FLAG -> D12 -> EP-M5 -> ADR 0007 amended
```

## Verification plan

### Abstract model

A source map `S` is a finite map from workspace-relative paths to byte strings
that are valid UTF-8. An intent `I` names a recipe, a set of declared files,
and a sequence of captures. A capture `c` names a path `c.path`, a half-open
range `[c.start, c.end)`, a file digest `c.base`, a snippet digest `c.snip`,
and a provenance value.

`prepare(S, I)` either returns validated captures or the first error, in a
fixed precedence order:

1. intent-level structure (known recipe, non-empty declared files, unique
   capture names, capture paths within the declared files, one compiler
   identity, and the recipe's required capture names and counts);
2. then, for each capture in intent order: provenance (macro-expansion-only is
   rejected), the path exists in `S`, `sha256(S[path]) = c.base`,
   `c.start <= c.end <= len(S[path])` with both offsets on character
   boundaries, and `sha256(S[path][c.start..c.end]) = c.snip`.

A file edit list `E = [(s_0, e_0, r_0), ..., (s_{n-1}, e_{n-1}, r_{n-1})]` over
a body `B` is well formed (`WF(B, E)`) when, for every `i`,
`s_i <= e_i <= len(B)`, both offsets are character boundaries,
`(s_i, e_i, r_i)` is not a no-op, and for every `i < n - 1`, `s_i < s_{i+1}` and
`e_i <= s_{i+1}`.

The specification of application is segment interleaving:
`apply(B, E) = B[0..s_0] ++ r_0 ++ B[e_0..s_1] ++ r_1 ++ … ++ r_{n-1} ++
B[e_{n-1}..len(B)]`.

Canonicalization sorts by `(s, e)` and then checks `WF`. An input that is not
`WF` after sorting is rejected with the first violation in sorted order.

### Invariants and lemmas

- `INV-CAPTURE-PRECEDENCE` (`REQ-CAPTURE`): `prepare` returns exactly the first
  failing check in the precedence order above, and never slices or hashes a
  range before validating it.
- `INV-NO-PANIC` (`REQ-CAPTURE`, `REQ-HARNESS`): `prepare`, `canonicalize`,
  and `apply` return typed errors and never panic on any input, including
  out-of-bounds, reversed, and mid-character ranges.
- `INV-CANONICAL-ORDER` (`REQ-HARNESS`): canonicalization is independent of
  input order. Any permutation of the same edits produces the same canonical
  plan or the same first violation.
- `INV-WF-SOUND-COMPLETE` (`REQ-HARNESS`): canonicalization accepts exactly
  the edit lists that are pairwise disjoint (under `D4`), in bounds, on
  boundaries, without no-ops, and without shared starts.
- `INV-FRAME` (`REQ-PIPELINE`): for `WF(B, E)`, every byte of `B` outside all
  edit ranges appears in `apply(B, E)` in its original order, and only
  replacement bytes are added.
- `INV-DETERMINISM` (`REQ-DETERMINISM`): for a deterministic backend, the
  raw plan bytes or the typed error and stable diagnostic data are identical
  across 100 fresh processes. The harness rejects any difference.
- `INV-FIXTURE-CONSISTENCY` (`REQ-FIXTURES`): for every primary fixture,
  applying the manifest's declared edits to `before/` yields `after/`
  byte-for-byte, and every capture's literal text, range, and digests agree.
- `L-ADJACENT-IMPLIES-PAIRWISE`: if starts are strictly increasing and each
  adjacent pair satisfies `e_i <= s_{i+1}`, then for all `i < j`, `e_i <= s_j`
  and `s_i < s_j`. This lemma justifies checking adjacent pairs only, and so
  carries `INV-WF-SOUND-COMPLETE`.
- `L-FORWARD-APPLY`: the production algorithm's shape, which walks edits
  forwards with a cursor and appends `B[cursor..s_i] ++ r_i`, then
  `B[e_last..]`, equals the segment-interleaving specification for every
  `WF(B, E)`. RFC 0007 §10.2 states the end-to-start formulation; the two are
  related by this lemma.
- `L-FRAME`: `INV-FRAME` follows from `L-FORWARD-APPLY`. Removing the
  replacement segments from the specification's output leaves exactly `B` with
  the edit ranges removed.

These lemma statements are hypotheses about the proof's shape. The Verus work in
`EP-M2` may merge, split, or replace them as the verification conditions
become concrete. Unproved statements stay here until then.

### Axioms (trusted, not verified here)

- `AXIOM-SHA256`: `sha2::Sha256` computes SHA-256 correctly and collisions do
  not occur in practice. Used by capture and fixture digest checks.
- `AXIOM-STR-BOUNDARY`: `str::is_char_boundary` and `str::get` agree with UTF-8
  character boundaries. Used by range validation.
- `AXIOM-RUSTC`: the pinned `rustc` on `nightly-2026-05-28` decides NLL and
  `-Zpolonius=next` acceptance. The compiler is the oracle for fixture
  outcomes; the spike does not reason about borrow checking.
- `AXIOM-CARGO-ENV`: Cargo honours `CARGO_ENCODED_RUSTFLAGS`,
  `CARGO_TARGET_DIR`, and `--offline` as documented, and a package with an empty
  `[workspace]` table is its own workspace root.
- `AXIOM-PROCESS`: a fresh operating-system process starts with fresh hash
  seeds and no state shared with earlier runs, so 100 fresh processes can
  expose order nondeterminism that a single process would hide.
- `AXIOM-SERDE`: `serde_json` serializes struct fields in declaration order and
  sequences in element order.

### Methods per obligation

- Obligation: `INV-CAPTURE-PRECEDENCE`.
  Method: an `rstest` table plus a `proptest` property. Rationale: the
  precedence order is a finite decision list, so a table names each rule. A
  property then checks combinations of faults that a table would miss. Domain:
  one row per error class (unknown recipe, missing capture, duplicate name,
  undeclared path, mixed compiler identity, macro-expansion-only, missing
  source, stale file, out-of-bounds, reversed, mid-character start,
  mid-character end, stale snippet), plus generated captures with zero to three
  injected faults. Artefact: `crates/spike_contract/src/capture/tests.rs` and
  `crates/spike_contract/tests/capture_precedence.rs`. Evidence: the focused
  `spike_contract` `capture` run listed in `Concrete steps`. These fail before
  `prepare_intent` exists (red) and pass after it does (green). Non-vacuity:
  the property's oracle is an independent list of `(predicate, error)` pairs
  evaluated in order. The generator records the class of each case with
  `prop_assert!` counters and must reach every fault class at least once in 256
  cases. A seeded fault that swaps the file-digest and range checks must make
  the property fail on a case with both faults.

- Obligation: `INV-NO-PANIC`.
  Method: `proptest` over arbitrary ranges, including multi-byte strings.
  Rationale: panics come from untested input combinations, and generated input
  finds them cheaply. Domain: source strings mixing ASCII and multi-byte
  characters (`é`, `日`, and `🦀`), with `start` and `end` drawn from
  `0..=len + 4`. Artefact: `crates/spike_contract/tests/no_panic.rs`. Evidence:
  the property passes. Clippy's `indexing_slicing` and `string_slice` denials
  stop the code from slicing unchecked. Non-vacuity: the generator must produce
  mid-character offsets, which is checked by a counter. A seeded fault that
  replaces `str::get` with direct slicing must panic under the property.

- Obligation: `INV-CANONICAL-ORDER` and `INV-WF-SOUND-COMPLETE`.
  Method: `proptest` against an independent O(n²) pairwise oracle. Rationale:
  the space of edit lists is large and order-sensitive, so it needs generated
  cases. The pairwise oracle shares no code with the adjacent-pair
  implementation. Domain: zero to eight edits over a body of up to 64 bytes,
  biased towards touching, nested, same-start, zero-width, and no-op edits,
  with shuffled input order. Artefact:
  `crates/spike_contract/tests/plan_properties.rs`. Evidence: the properties
  pass. Recorded class counts show every class was reached (empty plan,
  adjacent edits, zero-width edits, same-start conflict, strict overlap, out of
  bounds, mid-character, and no-op). Non-vacuity: two seeded faults must each
  be rejected. Allowing `e_i = s_{i+1} + 1` breaks the oracle comparison, and
  removing the same-start rule breaks it on a zero-width pair. Revert both and
  record the transcripts.

- Obligation: `INV-FRAME` and the executable side of `L-FORWARD-APPLY`.
  Method: `proptest` against a naive oracle, plus Verus for the model.
  Rationale: the property ties the compiled code to the specification, and the
  proof establishes the specification-level identity for all lengths. Domain:
  well-formed plans from the generator above. Artefact:
  `crates/spike_contract/tests/plan_properties.rs` (the oracle builds the
  output segment by segment from the definition) and
  `verus/rewrite_edit_plan.rs`. Evidence: the property passes.
  `make verus-rewrite-edit-plan` reports every function verified with zero
  errors. Non-vacuity: a seeded off-by-one in the cursor update must fail the
  property. For Verus, see the next entry.

- Obligation: `L-ADJACENT-IMPLIES-PAIRWISE`, `L-FORWARD-APPLY`, and `L-FRAME`.
  Method: Verus deductive proof. Rationale: these are unbounded statements over
  any number of edits and any body length. They carry the guarantee the harness
  relies on when it checks only adjacent pairs and applies edits in a single
  forward pass. Domain: `Seq<u8>` bodies and `Seq<Edit>` plans with `nat`
  offsets. UTF-8 boundaries are not modelled, because they are a separate check
  whose correctness is `AXIOM-STR-BOUNDARY`. Artefact:
  `verus/rewrite_edit_plan.rs`, a new `rewrite-edit-plan` group in
  `scripts/run-verus.sh` (also added to `all`), and a Makefile target
  `verus-rewrite-edit-plan`. Evidence: `make verus-rewrite-edit-plan` prints
  `verification results:: N verified, 0 errors`, where `N` is recorded at
  implementation time. Non-vacuity: the file contains a witness lemma that
  constructs a concrete two-edit plan over `"abcdef"` satisfying `WF`, and
  proves the expected output. This shows the precondition is inhabited and the
  theorems are not vacuous. A seeded fault weakens the adjacent condition to
  `e_i <= s_{i+1} + 1`; `L-ADJACENT-IMPLIES-PAIRWISE` must then fail to verify.
  The transcript is recorded and the fault reverted. The proof must contain no
  `assume`, `admit`, or `external_body`. A
  `grep -nE 'assume|admit|external_body' verus/rewrite_edit_plan.rs` check that
  prints nothing is part of the evidence.

- Obligation: `INV-DETERMINISM`.
  Method: an end-to-end test with real processes and test doubles. Rationale:
  nondeterminism across processes is the thing being detected, so only real
  processes exercise it (`AXIOM-PROCESS`). Domain: the four primary fixtures
  with the `replay` double, 100 runs each; one fixture with the `hash-order`
  double, which emits two edits in `HashSet` iteration order. Artefact:
  `crates/spike_harness/tests/determinism.rs`. Evidence: `replay` passes 100 of
  100 runs. `hash-order` is rejected with
  `PlanBytesDiverged { first_run: 0, diverging_run: k }` for some `k`.
  Non-vacuity: `hash-order` is the negative control. With two distinct keys,
  the probability that 100 runs agree by chance is about 2⁻⁹⁹. The test also
  checks that the `replay` double's raw plans contain at least one edit, so the
  comparison is not between empty outputs.

- Obligation: `INV-FIXTURE-CONSISTENCY`.
  Method: a manifest-driven check over every fixture, run by
  `make spike-rewrite-fixtures` and by the BDD suite. Rationale: the fixtures
  are the oracle for 14.1.2. If they are wrong, both backends are judged
  against a wrong answer. Domain: all four primary fixtures. Artefact:
  `crates/spike_harness/src/fixture/integrity.rs` and
  `crates/spike_harness/tests/features/fixture_corpus.feature`. Evidence: the
  command passes. A test copies a fixture into a temporary directory, changes
  one byte of `after/`, and asserts that the check fails, naming the file and
  offset. Non-vacuity: the corruption test is the negative control. A second
  test corrupts a capture's literal text and expects a literal-text mismatch.

- Obligation: the expected compiler outcomes for each fixture.
  Method: real compiler runs through the profile runner. Rationale:
  `AXIOM-RUSTC`. Only the compiler can confirm the outcomes. Domain: each
  fixture's `before/` and `after/` under both profiles, plus the capability
  canary. Artefact: `crates/spike_harness/src/profile/` and the BDD feature.
  Evidence: `make spike-rewrite-fixtures` prints the outcome table in
  `Purpose / big picture`. Non-vacuity: the canary must be rejected by `nll`
  and accepted by `polonius-next` before any fixture result counts, which shows
  both profiles actually differ on this host. `bor001`'s `after/` must be
  NLL-rejected, which shows the `nll` profile is not silently running Polonius.

### Formal verification decision

Verus is used for the edit-plan kernel only (`D9`). The kernel is contractual
logic that every later rewrite stage reuses: RFC 0007 §10.2 and RFC 0009 §20.2
both name non-overlap and faithful application as properties, and the harness's
design depends on two non-obvious claims, that checking adjacent pairs is
enough and that forward application matches the specification. Both are
unbounded in the number of edits, so tests can only sample them. A small pure
model exists, and the repository already has a closely related proof
(`verus/string_continuation_rewrite.rs`).

As with the repository's other Verus files, the proof covers an independent
model, not the compiled Rust. The gap between model and code is closed by the
`proptest` oracle properties, which run the real functions.

Kani is not used. Its strength here would be a bounded, exhaustive check of the
compiled `canonicalize` function for small plans. The `proptest` oracle
comparison, biased towards the boundary classes, already covers that ground for
the compiled code, and the Verus proof covers the unbounded statement. Adding
Kani would add a third harness without a property the other two leave open.

No proof obligations are assigned to capture validation, fixture loading,
process management, compiler invocation, diffs, or the report. Those are finite
decision lists or integration plumbing, which are proportionately covered by
tables, properties, and end-to-end tests.

### Suggested BDD features

Two feature files drive `EP-M3` and `EP-M4`. Both are run by `rstest-bdd` 0.6
from `crates/spike_harness/tests/`.

`spikes/rewrite-materialization/crates/spike_harness/tests/features/fixture_corpus.feature`
(`REQ-FIXTURES`, `EP-M3`):

```gherkin
Feature: Shared rewrite fixture corpus
  Scenario: Every primary fixture is internally consistent
    Given the primary fixture corpus
    When the fixture integrity check runs
    Then every fixture's declared edits reproduce its expected output exactly
    And every capture's text, range, and digests agree

  Scenario: A corrupted expected output is reported precisely
    Given a copy of fixture "own001-clone-only-borrow" whose expected output has one changed byte
    When the fixture integrity check runs
    Then the check fails naming file "src/lib.rs" in the expected output

  Scenario: The checker profiles are distinguishable on this toolchain
    Given the pinned toolchain
    When the capability probe runs
    Then the nll profile rejects the canary
    And the polonius-next profile accepts the canary

  Scenario: The conditional lookup fixture is Polonius-only
    Given fixture "bor001-conditional-lookup"
    When both checker profiles compile its expected output
    Then nll rejects it with error code "E0499"
    And polonius-next accepts it
```

`spikes/rewrite-materialization/crates/spike_harness/tests/features/backend_conformance.feature`
(`REQ-CAPTURE`, `REQ-PIPELINE`, `REQ-DETERMINISM`, `EP-M4`):

```gherkin
Feature: Backend conformance pipeline
  Scenario: A faithful backend passes every primary fixture
    Given the replay test backend
    When the pipeline runs every primary fixture with 100 fresh processes
    Then every fixture passes every gate
    And the report records review_required for "bor002-remove-reinsert"

  Scenario: A backend that ignores a stale capture is rejected
    Given the stale-blind test backend
    And fixture "own001-clone-only-borrow" with one capture's snippet digest altered
    When the pipeline runs that fixture
    Then the fixture fails because a stale capture was accepted

  Scenario: A nondeterministic backend is rejected before application
    Given the hash-order test backend
    When the pipeline runs fixture "own001-clone-only-borrow" with 100 fresh processes
    Then the fixture fails because raw plan bytes diverged
    And no rewritten source was produced

  Scenario: A backend that widens an edit is rejected before application
    Given the widening test backend
    When the pipeline runs fixture "own002-owned-param"
    Then the fixture fails because the edit-range set differs from the manifest

  Scenario: Overlapping edits are detected before application
    Given the overlapping test backend
    When the pipeline runs fixture "bor002-remove-reinsert"
    Then the fixture fails with an overlap between two edits in one file

  Scenario: A crashing backend is reported, not propagated
    Given the panicking test backend
    When the pipeline runs fixture "own001-clone-only-borrow"
    Then the fixture fails because the backend process exited abnormally
    And the harness itself exits normally
```

## Plan of work

The work runs in six milestones. Every milestone ends with the full gate
sequence in `Concrete steps`, and the code milestones follow
Red-Green-Refactor: write the failing test first, run it and see it fail for
the stated reason, write the smallest code that passes it, then tidy and rerun.

### `EP-M0` workspace and inheritance probe (prototype)

Question: does a standalone workspace under `spikes/` build cleanly, and which
root configuration does it inherit? Create a throwaway workspace under
`target/scratch/spike-probe/` that mirrors the planned layout (one package and
one fixture-style package with an empty `[workspace]` table), and record:

1. `cargo metadata --manifest-path <probe>/Cargo.toml --no-deps` succeeds and
   reports only the probe's packages.
2. `cargo fmt --manifest-path <probe>/Cargo.toml --all -- --check` applies the
   root `.rustfmt.toml`. Check with a `use` statement that only
   `imports_granularity = "Crate"` would reformat.
3. Whether `cargo clippy` in the probe reads the root `clippy.toml`. Check with
   a function whose line count exceeds the root `too-many-lines-threshold` if
   one is set, or with `allow-expect-in-tests`.
4. Whether `cargo nextest run` in the probe reads the root
   `.config/nextest.toml` (expected: no).
5. A fixture-style package copied under the probe's `target/` builds with
   `cargo check --offline` and does not report "believes it's in a workspace".

Write the transcript into `Artefacts and notes`, then delete the probe
directory. Go on to `EP-M1` if items 1, 2, and 5 hold. If item 1 or 5 fails,
stop and escalate, because `D2` and `D6` depend on them. Items 3 and 4 only
decide whether the spike's `clippy.toml` and `.config/nextest.toml` are
mandatory or just explicit; both are created regardless.

### `EP-M1` spike workspace, contract model, and capture validation

Assigned: `REQ-ISOLATION`, `REQ-CONTRACT`, `REQ-CAPTURE`, `GAP-TYPES`,
`GAP-GATES`, `GAP-EOL`; decisions `D2`, `D3`, `D7`, `D8`, `D10`, `D11`.

1. Create `spikes/rewrite-materialization/Cargo.toml` with
   `[workspace] members = ["crates/*"]`, `resolver = "2"`, a
   `[workspace.package]` table (edition 2024, `publish = false`, licence ISC),
   `[workspace.dependencies]` for the authorized crates, and
   `[workspace.lints]` tables copied from the root `Cargo.toml`. Add
   `spikes/rewrite-materialization/clippy.toml` (root settings plus
   `disallowed-types` for `std::collections::HashMap` and
   `std::collections::HashSet`) and
   `spikes/rewrite-materialization/.config/nextest.toml` (a default profile
   with a 300-second `slow-timeout`, and a `fixture-compiler` test group with
   `max-threads = 2` and a 900-second allowance for compiler-backed tests).
2. Create `crates/spike_contract` (library) and `crates/spike_harness`
   (library plus a `rewrite-spike` binary). The harness binary at this stage
   only parses its subcommands and exits with a usage message.
3. Red: add a lint-parity test in `crates/spike_harness/tests/lint_policy.rs`
   that parses the root `Cargo.toml` and the spike `Cargo.toml` with `toml` and
   asserts that every root `workspace.lints` entry is present with the same
   level in the spike. Run it before copying the tables, to see it fail.
4. Red: in `crates/spike_contract`, write unit tests for the newtypes:
   `WorkspaceRelativePath::new` rejects absolute paths, `..`, empty strings,
   backslashes, and trailing separators, and accepts `src/lib.rs`;
   `Sha256Digest` parses and prints 64 lower-case hexadecimal characters and
   rejects anything else; `ByteRange` construction is unchecked (it is just two
   offsets) but `ByteRange::validate_in(source)` returns
   `RangeFault::{Reversed, OutOfBounds, StartNotCharBoundary,
   EndNotCharBoundary}`.
5. Green: implement the model types listed under `Interfaces and dependencies`
   in `crates/spike_contract/src/model/` (paths, digests, ranges, identities,
   provenance, recipe, captures, and intent). Derive `serde::Serialize` and
   `Deserialize` behind no feature; the contract may depend on `serde` but not
   on `serde_json` or `toml`.
6. Red: write the `INV-CAPTURE-PRECEDENCE` table and property, and the
   `INV-NO-PANIC` property, against `prepare_intent`.
7. Green: implement `prepare_intent`, `PreparedIntent`, and `ValidatedCapture`
   in `crates/spike_contract/src/capture/`. Implement the recipe capture
   schemas in `crates/spike_contract/src/recipe.rs`. The schemas declare, for
   each recipe, the required capture names, how many of each, and the
   `ExpectedSyntax` each must have. The exact names come from the fixture
   designs in `EP-M3`; this step defines them for the four recipes and tests
   them with synthetic intents.
8. Add `SpikeBackend` and `SpikeError` in `crates/spike_contract/src/backend.rs`
   and `crates/spike_contract/src/error.rs`. `SpikeError` provides
   `stable_diagnostic()`, which returns a `StableDiagnostic` with a dotted key
   such as `spike.capture.stale.snippet`, the capture name, and the path, and
   nothing else.
9. Add the Makefile targets from `D10` and call them from `fmt`, `check-fmt`,
   `lint`, and `test`. Add `spikes/rewrite-materialization/fixtures/** -text` to
   `.gitattributes` with a comment explaining why. Add a
   `make spike-rewrite-test` step to `linux-full` in
   `.github/workflows/ci.yml`. Run `make test-workflow-contracts` and update
   the lane expectations it reports, within the CI tolerance.
10. Refactor: keep each file under 400 lines. Check `cargo tree` for forbidden
    crates.

Acceptance: the contract tests pass. `make check-fmt`, `make lint`, and
`make test` run the spike and pass.
`cargo tree --manifest-path
spikes/rewrite-materialization/Cargo.toml | grep -E 'ast-grep|ra_ap'`
prints nothing. The root `cargo metadata --no-deps` lists no spike package.

### `EP-M2` edit-plan kernel and Verus proofs

Assigned: `REQ-HARNESS` (sorting, overlap, application, range-set comparison,
untouched-byte check), `GAP-PLAN-SEMANTICS`; decisions `D4` and `D9`.

The kernel lives in `spike_contract`, not in the harness, because 14.2.1 will
lift it into the production model and because backends' own tests in 14.1.2
will use it. The harness is still the only caller in the pipeline.

1. Red: write `rstest` tables for `canonicalize_plan` covering each violation
   (`OutOfBounds`, `NotCharBoundary`, `NoOpEdit`, `SharedStart`, `Overlap`,
   `DuplicateFile`, `UndeclaredFile`), and the accepted shapes (empty plan,
   touching edits, a zero-width insertion followed by a replacement starting
   later, and unsorted input that becomes sorted).
2. Red: write the `INV-CANONICAL-ORDER`, `INV-WF-SOUND-COMPLETE`, and
   `INV-FRAME` properties with their oracles.
3. Green: implement `TextEdit`, `FileEdits`, `TextEditPlan`, `CanonicalPlan`,
   `canonicalize_plan`, `apply_file_edits`, `edit_range_set`, and
   `untouched_bytes_preserved` in `crates/spike_contract/src/plan/`.
   `apply_file_edits` takes only a `CanonicalPlan` entry, so unvalidated edits
   cannot be applied.
4. Write `verus/rewrite_edit_plan.rs` with the model, the three lemmas, and the
   witness. Add the `rewrite-edit-plan` group to `scripts/run-verus.sh`
   (including in `all`) and the `verus-rewrite-edit-plan` Makefile target. Run
   the seeded faults described in `Verification plan` and record them.
5. Refactor, keeping the forward-cursor shape of `apply_file_edits` identical
   to the Verus model's `forward_apply`. Put a doc comment on both pointing at
   each other.

Acceptance: the properties pass with class counts recorded.
`make verus-rewrite-edit-plan` and `make verus` pass. The seeded-fault
transcripts are in `Artefacts and notes`. The standard gates pass.

### `EP-M3` primary fixtures, fixture integrity, and checker profiles

Assigned: `REQ-FIXTURES`, `GAP-NLL-FLAG`, `GAP-EOL`; decisions `D5`, `D6`,
`D7`, and `D8`.

Each fixture is a library crate whose public function exercises the private
code, so that it compiles warning-free.

- `fixtures/own001-clone-only-borrow/`: a function that clones a `String` into
  a local, then passes `&local` to two read-only helpers, with a comment line
  and a blank line between the binding and the first use. Captures: the clone
  binding statement, each use of the local, and the original place. Expected
  edits: delete the binding statement, including its line, and replace each use
  with the original place. The comment and blank line stay. Outcomes: `before`
  and `after` both accepted by both profiles.
- `fixtures/own002-owned-param/`: `src/lib.rs` declares a private
  `fn render_label(label: String) -> usize` with a doc comment and an
  `#[inline]` attribute, plus `mod alpha; mod beta;`. `src/alpha.rs` and
  `src/beta.rs` each call `crate::render_label(name.clone())`, one of them
  twice. Captures: the parameter type, and each `.clone()` argument at each
  call site. Expected edits: `String` becomes `&str`, and each `name.clone()`
  becomes `&name`. Three files are edited and the doc comment, attribute, and
  imports are unchanged. Outcomes: accepted by both profiles, before and after.
- `fixtures/bor001-conditional-lookup/`: a function returning
  `&mut String` from a `HashMap<u32, String>` that checks `contains_key`, then
  calls `get_mut(...).unwrap()` or inserts and re-looks-up, with a comment in
  each branch. Captures: the statement range, the map place, the key, and the
  default value. The expected edit replaces the statement range with
  `if let Some(value) = map.get_mut(&key) { return value; }` followed by
  `map.entry(key).or_default()`, keeping both branch comments. Outcomes:
  `before` is accepted by both profiles; `after` is rejected by `nll` with
  `E0499` and accepted by `polonius-next`. The planning probe already confirmed
  this shape on the pinned nightly.
- `fixtures/bor002-remove-reinsert/`: a function returning `Result<(), Error>`
  that removes a value from a `HashMap`, mutates it under a comment, calls a
  fallible helper with `?` nearby, and reinserts the value. Captures: the
  remove statement, the mutation, and the reinsert statement. The expected
  edits delete the remove and reinsert statements and rewrite the mutation to
  go through `map.get_mut(&key)`. The fixture states in its manifest that the
  helper call keeps its position relative to the mutation, so evaluation order
  is unchanged. Outcomes: accepted by both profiles before and after. Manifest
  metadata: `confidence = "review_required"`.

The `.unwrap()` in `bor001`'s `before/` is fixture data, not spike code; the
spike's lints do not apply to fixture files.

Steps:

1. Red: write `fixture_corpus.feature` and its step definitions, and the
   integrity unit tests, including the one-byte corruption controls. Run them
   with no fixtures present and see them fail.
2. Green: implement the manifest model
   (`crates/spike_harness/src/fixture/ manifest.rs`, parsed with `toml`), the
   loader (`loader.rs`, using `cap_std::fs_utf8` to read `before/` and `after/`
   trees into `BTreeMap<WorkspaceRelativePath, String>`), and the integrity
   check (`integrity.rs`). The check verifies every file digest listed in the
   manifest, every capture's range, digests, and literal text, and that
   applying the declared edits (through `canonicalize_plan` and
   `apply_file_edits`) reproduces `after/` exactly. It also verifies that
   `after/` contains no file that `before/` lacks unless the manifest declares
   it.
3. Author the four fixtures. Use `rewrite-spike fixtures --explain <id>` to
   print each declared range's bytes while authoring. The `--explain` mode is
   implemented in this step and has its own snapshot test.
4. Red: write unit tests for the profile runner with a fake command executor
   (a closure seam returning canned exit statuses and JSON diagnostics). Cover
   flag selection for both toolchain shapes (no-flag NLL, as on the pinned
   nightly, and `-Zpolonius=off` NLL, as on later nightlies), the `unavailable`
   outcome, and error-code extraction.
5. Green: implement `crates/spike_harness/src/profile/`. `ProfileRunner` copies
   a package into `target/spike-runs/<run-id>/<fixture>/<side>/` (cleared at
   start), runs `cargo check --offline --message-format=json` with
   `CARGO_TARGET_DIR` set to a shared directory under the spike's `target/`,
   removes `RUSTFLAGS`, `RUSTDOCFLAGS`, and `CARGO_ENCODED_RUSTFLAGS` from the
   child environment, sets `CARGO_ENCODED_RUSTFLAGS` to the profile's flags,
   and leaves `RUSTC_WRAPPER` untouched. It records acceptance and the sorted,
   de-duplicated set of error codes. The canary is written into its own package
   at run time.
6. Wire `rewrite-spike fixtures` to run integrity and profile checks for all
   fixtures and print the outcome table. Add the `spike-rewrite-fixtures`
   Makefile target.

Acceptance: `make spike-rewrite-fixtures` prints the table in
`Purpose / big picture` and exits zero. The BDD feature passes. The corruption
controls fail as specified. The standard gates pass.

### `EP-M4` backend pipeline, determinism runner, report, and conformance kit

Assigned: `REQ-PIPELINE`, `REQ-DETERMINISM`, `REQ-REPORT`, `REQ-CAPTURE`
(observed through backends); decision `D11`.

1. Red: write `backend_conformance.feature` and its steps, and unit tests for
   each pipeline stage with fakes.
2. Green: implement, in `crates/spike_harness/src/`:
   - `backend_registry.rs`: maps a backend name to a `Box<dyn SpikeBackend>`.
     The production registry is empty in this item. Test doubles are
     registered only when the `test-doubles` Cargo feature is enabled; the
     feature is enabled by the spike's test targets and never by default.
   - `doubles/` (behind `test-doubles`): `replay` (calls `prepare_intent`, then
     returns the manifest's expected edits, in reverse order to exercise
     harness sorting), `stale-blind` (skips `prepare_intent`), `hash-order`,
     `widening` (extends one edit by one byte), `overlapping` (duplicates an
     edit's start), and `panicking`.
   - `process_runner.rs`: spawns
     `rewrite-spike materialize --backend <name> --fixture <dir>` with bounded
     output and a deadline (`D11`). The child loads the fixture, calls the
     backend once, and writes a `MaterializeOutcome` (either the raw plan or
     the stable diagnostic) as JSON to standard output.
   - `determinism.rs`: runs `N` fresh processes and compares raw bytes. `N` is
     `DETERMINISM_RUNS = 100` in the pipeline; unit tests inject smaller values
     through the internal function.
   - `pipeline.rs`: the 11 steps of ADR 0007 §Validation pipeline as a flat
     sequence of named stage functions, each returning a `StageOutcome`, and
     stopping at the first failing gate. Expected rejections end after step 5.
     Formatting runs `cargo fmt --check` on the rewritten copy and records
     differences as diagnostics only.
   - `diff.rs`: unified diffs with `similar`, with paths written as
     `a/<path>` and `b/<path>`.
   - `report.rs`: `SpikeReport` with `schema_version`, backend name, toolchain
     identity, probe results, and per-fixture results in manifest order,
     serialized with `serde_json::to_string_pretty`.
   - `conformance.rs`: `run_conformance(backend_name)`, which runs every
     fixture through the pipeline and returns the report. The 14.1.2 backends
     call this from their own tests.
3. Wire `rewrite-spike compare --backend <name> --report <path>`. With the
   empty production registry it exits with status 2 and the message
   `no backend named <name> is registered`.
4. Add insta snapshots of the `replay` report for all four fixtures, and of the
   unified diff for `own002-owned-param`. Pair each snapshot with semantic
   assertions on the gate outcomes, so a snapshot update cannot hide a gate
   change.
5. Add the end-to-end tests: `rewrite-spike compare` with the `replay` double
   writes a report whose gates all pass; with `hash-order` the report records
   the divergence; `rewrite-spike compare --backend missing` exits 2.

Acceptance: the conformance feature passes, the determinism test passes for
`replay` and rejects `hash-order`, snapshots are stable across two consecutive
runs, and the standard gates pass.

### `EP-M5` documentation, reconciliation, and roadmap closure

1. ADR 0007: in §Shared spike interface, add a subsection "Contract
   refinements from the spike contract work (14.1.1)" recording `D3` to `D8`
   with their rationale. In §Outstanding decisions, record the open
   `borrow.remove_then_reinsert` kind for RFC 0009 and note that the NLL flag
   depends on the toolchain. Keep the status `Proposed`.
2. `spikes/rewrite-materialization/README.md`: purpose, layout, the contract
   (trait, input and output types, errors, and stable diagnostic keys), the
   validation precedence, the plan semantics, how to add a fixture, how a
   backend uses `prepare_intent` and the conformance kit, the toolchain note for
   `bor001`, and the reuse policy.
3. `docs/developers-guide.md`: a "Rewrite-materialization spike" section with
   the Make targets, the fixture authoring workflow, and the reuse policy
   (nothing outside `spikes/` may depend on the spike; 14.2.1 lifts the
   contract into `whitaker_rewrite_model` deliberately rather than by import).
4. `docs/repository-layout.md`: add the `spikes/` row between `skills/` and
   `src/`, and a `verus/rewrite_edit_plan.rs` mention in the verification
   assets list.
5. `docs/contents.md`: no new document is added, so no change is needed unless
   the README is judged index-worthy. ADR 0007's existing entry stays.
6. Run the stale-claim search
   `grep -rn "spikes/rewrite-materialization\|SpikeBackend" docs --include=*.md
   | grep -v docs/execplans` and reconcile every hit with the implementation.
7. Mark roadmap item 14.1.1 done in `docs/roadmap.md`.
8. Fill in `Outcomes & retrospective` and set this plan's status to
   `COMPLETE`.

## Milestones and plateaus

- `EP-M0`: probe findings recorded; nothing committed except this plan's
  update. Recovery: delete `target/scratch/spike-probe/`. Compatibility: none.
- `EP-M1`: the spike workspace exists, is gated locally and in CI, and its
  contract and capture validator are tested. Requirements: `REQ-ISOLATION`,
  `REQ-CONTRACT`, `REQ-CAPTURE`. Evidence: contract tests, the lint-parity
  test, and `cargo tree`. Conformance check:
  - capture fields match ADR 0007 §Shared spike interface;
  - no backend type in the contract;
  - no root member change;
  - no new root dependency;
  - `.gitattributes` override present;
  - trace links current.
  Recovery: revert the milestone commits. The root workspace is untouched apart
  from the Makefile, `.gitattributes`, and CI wiring. Remaining gaps: the plan
  kernel, fixtures, and pipeline. Compatibility: none.
- `EP-M2`: the edit-plan kernel is implemented, property-tested, and proved.
  Requirements: `REQ-HARNESS` (kernel part). Evidence: the properties, Verus
  output, and seeded-fault transcripts. Conformance check:
  - `D4` semantics are implemented exactly;
  - the Verus model mirrors the forward-cursor shape;
  - no `assume`, `admit`, or `external_body`;
  - trace links current.
  Recovery: revert. Remaining gaps: fixtures and pipeline. Compatibility: none.
- `EP-M3`: four consistent fixtures with confirmed compiler outcomes, and the
  `make spike-rewrite-fixtures` command. Requirements: `REQ-FIXTURES`.
  Evidence: the command output and the BDD feature. Conformance check:
  - each fixture meets its ADR 0007 bullet list;
  - no fixture-specific harness code;
  - `bor001` NLL rejection observed;
  - trace links current.
  Recovery: revert. A fixture that cannot meet its ADR bullets is an
  architecture escalation. Remaining gaps: pipeline and report. Compatibility:
  none.
- `EP-M4`: the full pipeline, determinism runner, report, and conformance kit,
  proved against doubles. Requirements: `REQ-PIPELINE`, `REQ-DETERMINISM`,
  `REQ-REPORT`. Evidence: the conformance feature, determinism test, snapshots,
  and end-to-end tests. Conformance check:
  - the 11 pipeline steps map one-to-one to stage functions;
  - rejections stop after step 5;
  - the production registry is empty;
  - trace links current.
  Recovery: revert. Remaining gaps: documentation. Compatibility: none.
- `EP-M5`: documentation reconciled and the roadmap entry closed. Evidence: the
  Markdown gates and the stale-claim search. Compatibility: none.

## Concrete steps

All commands run from the repository root. Every gate is run once, in this
order, with `pipefail`, and with output captured for later reading:

```bash
set -o pipefail
BR=$(git branch --show-current)
make check-fmt 2>&1 | tee "/tmp/check-fmt-whitaker-${BR}.out"
make lint 2>&1 | tee "/tmp/lint-whitaker-${BR}.out"
make test 2>&1 | tee "/tmp/test-whitaker-${BR}.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-whitaker-${BR}.out"
make nixie 2>&1 | tee "/tmp/nixie-whitaker-${BR}.out"
```

`EP-M2` adds:

```bash
make verus-rewrite-edit-plan 2>&1 | tee "/tmp/verus-whitaker-${BR}.out"
make verus 2>&1 | tee "/tmp/verus-all-whitaker-${BR}.out"
```

`EP-M3` onwards adds:

```bash
make spike-rewrite-fixtures 2>&1 | tee "/tmp/spike-fixtures-whitaker-${BR}.out"
```

Focused commands while iterating:

```plaintext
Working directory: repository root
Setup:    SPIKE=spikes/rewrite-materialization/Cargo.toml
Red:      cargo nextest run --manifest-path "$SPIKE" -p spike_contract capture
          -> fails: cannot find function `prepare_intent` (EP-M1), then assertion failures until implemented
Green:    same command -> all capture tests pass
Red:      cargo nextest run --manifest-path "$SPIKE" -p spike_contract plan
          -> fails: cannot find function `canonicalize_plan` (EP-M2)
Green:    same command -> all plan tests and properties pass
Red:      cargo nextest run --manifest-path "$SPIKE" -p spike_harness --features test-doubles fixture_corpus
          -> fails: fixture corpus not found (EP-M3)
Green:    same command -> 4 scenarios pass
Red:      cargo nextest run --manifest-path "$SPIKE" -p spike_harness --features test-doubles backend_conformance
          -> fails: unknown backend `replay` (EP-M4)
Green:    same command -> 6 scenarios pass
```

`make test`'s normal run on the default nextest profile skips installer
behaviour tests. That is unrelated to this item and is not a gate failure.

Commit after each green step that passes the gates, with an imperative subject
line and a body explaining what and why. Run the documentation-only gates
(`make markdownlint`, `make nixie`, `make check-fmt`) for documentation-only
commits.

## Validation and acceptance

The item is complete when all of the following hold:

- `make spike-rewrite-fixtures` exits zero and prints the outcome table in
  `Purpose / big picture`, including `nll=reject(E0499)` for `bor001`'s `after`.
- `make check-fmt`, `make lint`, and `make test` pass, and their logs show the
  spike targets running.
- The `backend_conformance.feature` scenarios pass: `replay` passes every gate
  for every fixture across 100 processes, and each faulty double fails at the
  stated gate.
- `make verus-rewrite-edit-plan` reports zero errors, and the seeded fault was
  rejected before being reverted.
- The proptest properties pass and their recorded class counts show every
  class reached.
- `cargo tree --manifest-path spikes/rewrite-materialization/Cargo.toml`
  shows no `ast-grep` or `ra_ap` crate, and root `cargo metadata --no-deps`
  lists no spike package.
- The CI `linux-full` job runs `make spike-rewrite-test` on the PR and passes.
- Roadmap item 14.1.1 is marked done.

Quality criteria:

- Tests: every new unit, property, BDD, snapshot, and end-to-end test passes.
- Verification: `L-ADJACENT-IMPLIES-PAIRWISE`, `L-FORWARD-APPLY`, and `L-FRAME`
  are discharged by Verus (or their documented refinements), and every
  invariant in `Verification plan` has passing evidence with its non-vacuity
  control.
- Lint: `make lint` passes, with the spike's Clippy run using the copied root
  policy and the `disallowed-types` addition.
- Performance: no target. `make test` should not grow by more than five
  minutes on an idle host. Report the measured difference.
- Security: fixture builds run offline with no dependencies and only inside
  the spike's `target/` directory.

## Idempotence and recovery

Every step can be rerun. Fixture runs clear their own run directory under
`spikes/rewrite-materialization/target/spike-runs/` at start, and leave it in
place afterwards for inspection.
`cargo clean --manifest-path spikes/rewrite-materialization/Cargo.toml` removes
all spike build output. Seeded faults for non-vacuity checks are applied as
uncommitted edits, recorded, and reverted with `git checkout -- <file>` before
the next commit. Any `proptest-regressions/` file a property creates is
reviewed: a genuine regression is kept and committed, and one created by a
seeded fault is deleted.

## Artefacts and notes

Planning probe (reconnaissance, 2026-11-16, before approval; a scratch crate
under `target/scratch/polonius-probe/` that is not part of the plan's
deliverables):

```plaintext
rustc 1.98.0-nightly (57d06900f 2026-05-27)
canary [default]          REJECT  error[E0499]: cannot borrow `*a` as mutable more than once at a time
canary [-Zpolonius=off]   REJECT  error: incorrect value `off` for unstable option `polonius`
                                  - either no value or `legacy` (the default), or `next` was expected
canary [-Zpolonius=next]  ACCEPT
fixture3 [default]        REJECT  error[E0499]: cannot borrow `*map` as mutable more than once at a time
fixture3 [-Zpolonius=off] REJECT  (same invalid-value error)
fixture3 [-Zpolonius=next] ACCEPT
```

`fixture3` was this shape, which is the planned `bor001` `after` form:

```rust
fn entry_or_default(map: &mut HashMap<u32, String>, key: u32) -> &mut String {
    if let Some(value) = map.get_mut(&key) {
        // Existing entry: hand out the borrow directly.
        return value;
    }
    // Missing entry: insert, then borrow the new value.
    map.entry(key).or_default()
}
```

Record the `EP-M0` probe transcript, seeded-fault transcripts, Verus output,
property class counts, and gate timings here as they are produced.

## Interfaces and dependencies

### Packages

- `spikes/rewrite-materialization/crates/spike_contract` (library
  `spike_contract`). Owns the contract. Dependencies: `serde` (derive), `sha2`,
  and `thiserror`. Dev-dependencies: `rstest`, `googletest`,
  `pretty_assertions`, and `proptest`.
- `spikes/rewrite-materialization/crates/spike_harness` (library
  `spike_harness` and binary `rewrite-spike`). Owns the harness. Dependencies:
  `spike_contract`, `serde`, `serde_json`, `toml`, `sha2`, `thiserror`,
  `camino`, `cap-std` (with `fs_utf8`), and `similar`. Dev-dependencies:
  `rstest`, `rstest-bdd`, `rstest-bdd-macros`, `googletest`,
  `pretty_assertions`, `insta` (with `json`), `proptest`, and `tempfile`.
  Feature: `test-doubles`, off by default.

All requirements are caret requirements matching the root workspace's versions.
No dependency is added to the root workspace. Authorized dependencies are
exactly those listed here.

### Contract types (`spike_contract`)

The signatures below are the target shape. Field names may change during
implementation only if the change is recorded in `Decision log` and the README.

```rust
/// A `/`-separated path relative to the fixture or workspace root.
pub struct WorkspaceRelativePath(String);

/// A SHA-256 digest held as 32 bytes and written as lower-case hexadecimal.
pub struct Sha256Digest([u8; 32]);

/// A half-open byte range `[start, end)` within one source string.
pub struct ByteRange { pub start: usize, pub end: usize }

pub struct CompilerIdentity { pub rustc_commit_hash: String }

pub enum SemanticNodeIdentity {
    Item { def_path: String },
    Expression { owner_def_path: String, syntax_fingerprint: String },
}

pub enum ExpansionProvenance { SourceAuthored, EditableMacroCallSite, MacroExpansionOnly }

pub enum ExpectedSyntax {
    LetStatement, ExpressionStatement, StatementRange, PathExpression,
    MethodCallExpression, FunctionParameterType, Expression,
}

pub struct RecipeId { pub kind: RecipeKind, pub version: u16 }

pub enum RecipeKind {
    CloneOnlyUsedByBorrow, OwnedParamCausesClone, LookupThenRelookup, RemoveThenReinsert,
}

pub struct FileIntent { pub path: WorkspaceRelativePath, pub base_file_sha256: Sha256Digest }

pub struct SemanticCapture {
    pub name: String,
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

pub fn canonicalize_plan(plan: &TextEditPlan, intent: &SpikeIntent, sources: &SourceMap)
    -> Result<CanonicalPlan, PlanViolation>;
pub fn apply_file_edits(source: &str, edits: &CanonicalFileEdits) -> String;
```

`SpikeError` variants: `MalformedIntent { reason }`,
`MacroExpansionOnly { capture }`, `MissingSource { capture, path }`,
`StaleCapture { capture, kind: StaleKind::{File, Snippet} }`,
`InvalidCaptureRange { capture, fault: RangeFault }`,
`UnexpectedSyntax { capture, expected }`, `AmbiguousSyntax { capture }`, and
`UnsupportedRecipeVariant { recipe, detail }`. The last three are raised only
by backends; the contract defines them so 14.1.2 cannot invent incompatible
ones. Every variant maps to one stable dotted diagnostic key.

`PlanViolation` variants: `OutOfBounds`, `NotCharBoundary`, `NoOpEdit`,
`SharedStart`, `Overlap`, `DuplicateFile`, and `UndeclaredFile`, each naming
the path and the offending edit indices in canonical order.

### Reuse policy

`spike_contract` and `spike_harness` exist only for the ADR 0007 spike. Only
packages inside `spikes/rewrite-materialization/` may depend on them. Backend
packages added in 14.1.2 may depend on `spike_contract` only. No root-workspace
crate may depend on any spike package. When 14.2.1 defines the production
`whitaker_rewrite_model`, it should lift the validated parts of the contract,
especially the `D4` plan semantics, the edit-plan kernel, and the Verus proof,
by deliberate copy and review, not by depending on the spike. ADR 0007's spike
exit artefacts then archive or remove the spike.

## Revision note

- 2026-11-16: initial draft from reconnaissance of ADR 0007, RFCs 0007 to 0009,
  repository gates and CI, past ExecPlan budgets, and the pinned nightly's
  borrow-checker behaviour. Awaiting design review and user approval.

[^lsp]: Language Server Protocol Specification 3.17, `TextEdit` and
    `TextDocumentEdit`:
    <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/>.
