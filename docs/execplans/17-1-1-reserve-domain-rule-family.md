# Reserve the `DOMAIN` rule family and integrate it with experimental selection

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (revised after expert design review; awaiting approval of the
plan and of decisions D-1 to D-4; blocked on roadmap items 3.6.1 and 3.6.2)

Roadmap item: 17.1.1 in `docs/roadmap.md`, governed by the normative
`docs/roadmap-phase-17-domain-amendment.md`.

## Purpose / big picture

Whitaker is growing a family of domain-modelling lints, described in RFC 0011
("Validate not, only parse") and RFC 0012 with its amendment. Every rule in
that family will carry a stable code beginning with `DOMAIN`. Users will opt in
with `whitaker check --experimental --select DOMAIN` or the equivalent
`whitaker.toml` settings. Before the first rule exists, the family itself must
be registered so that:

1. the five numeric bands for `DOMAIN` rule codes are fixed in code, not only
   in prose, so that no later rule can take a code from a gap between bands or
   from the wrong band;
2. the selector `DOMAIN` is accepted and explains itself when it enables
   nothing, while a code such as `DOMAIN100` (in a gap) or `DOMAIN104`
   (reserved but not yet shipped) is rejected with a message that says which of
   the two it is; and
3. registering the family is invisible to every existing user. Any
   configuration that was valid before this change resolves to the same rules
   for all non-`DOMAIN` families afterwards. The only way to gain a `DOMAIN`
   rule is to name the family or one of its codes, or to select `ALL` with
   experimental mode on.

Point 3 is the contract this item exists to establish. The plan proves it in
Verus (obligation `O-CONS`, in `Verification plan`) and checks the same
relation exhaustively on the production code over a bounded scope.

After this change, the following `whitaker.toml` loads and validates. It
enables no `DOMAIN` rule yet, and resolution reports that the `DOMAIN` family
has no implemented rules:

```toml
[lint]
experimental = true
select = ["DEFAULT", "DOMAIN"]
```

If the same file sets `experimental = false`, resolution instead reports that
`DOMAIN` selects only experimental rules and experimental mode is off. Once a
`DOMAIN` rule ships, that notice is what stops a CI job that forgot
`--experimental` from passing while running nothing.

Terms used throughout:

- *Rule code*: a stable identifier: a family name of uppercase ASCII letters
  followed by exactly three decimal digits, such as `PAN001` or `DOMAIN101`.
- *Family*: the letter part of a rule code. A family selector selects every
  rule whose family is *equal* to it. It never matches on string prefixes, so
  `DO` is not a family, and `DOC` does not select `DOMAIN001`.
- *Banded family*: a family whose numbers are allocated in declared intervals
  called *bands*. Each band has a purpose key, used in documentation and error
  messages, and a rule origin. `DOMAIN` is the first banded family. Families
  registered by roadmap item 3.6.1 are *unbanded*, and nothing about them
  changes.
- *Gap*: a number from `000` to `999` that lies in no band of a banded family.
  For `DOMAIN`, the gaps are `000`, `100`, and `200` to `899`.
- *Reserved code*: a code inside a band that no shipped rule uses yet, such as
  `DOMAIN104` today.
- *Rule origin*: `LintPass` for a rule implemented as a Dylint lint pass, or
  `WorkspaceAggregate` for a grouped report computed by `whitaker check`
  aggregation. Only the `DOMAIN900` to `DOMAIN999` band holds
  `WorkspaceAggregate` rules. Origin is not a severity level. RFC 0012 calls
  some lint-pass rules "report-only" (for example `DOMAIN108 invalid_default`);
  that is a per-rule level and is independent of origin.
- *Catalogue*: the registry of families and rules that selector resolution
  consults. Roadmap item 3.6.1 introduces it.
- *Experimental mode*: the boolean merged from `[lint].experimental`,
  `WHITAKER_LINT_EXPERIMENTAL`, and `--experimental`.
- *Curated experimental set*: the experimental rules that a bare
  `--experimental` (with no explicit `select`) adds to `DEFAULT`. Roadmap item
  3.6.2 defines it.
- *Explicit opt-in*: a rule that is experimental, not in `DEFAULT`, and not in
  the curated experimental set. Every `DOMAIN` rule is explicit opt-in until it
  is promoted individually (decision D-2).

## Constraints

- Roadmap items 3.6.1 (rule codes, families, `DEFAULT`/`ALL`) and 3.6.2
  (selector precedence and curated `--experimental`) must be merged to `main`
  before any production code in this plan is written. This plan builds on their
  catalogue, selector, and resolution code; it must not create a parallel
  registry or resolver. EP-M0 enforces this.
- Rule codes, family names, band boundaries, and selector spellings are stable,
  language-neutral identifiers and are never translated
  (`docs/whitaker-cli-design.md`: "Command names, rule codes, manifest keys,
  and JSON fields are stable and are never translated").
- The bands are exactly those of RFC 0011 §Rule-code allocation, Table 1, as
  restated by the Phase 17 amendment: `001`–`099`, `101`–`149`, `150`–`179`,
  `180`–`199`, and `900`–`999`. Changing a boundary requires an RFC change.
- `DOMAIN` must not enter `DEFAULT`. `ALL` includes `DOMAIN` rules only when
  experimental mode is on (RFC 0011 §Display name and selector).
- Token handling for every family that 3.6.1 registered must not change. An
  insta snapshot taken at EP-M0 is the reference (obligation
  `O-PARSE-PRESERVE`).
- No lint crate, no `whitaker::domain` compiler lint group, and no Fluent
  message is added. The selector does not depend on a compiler lint group
  existing (RFC 0011 §Category, not monolith). The three hand-maintained
  lint-crate lists (`installer/src/resolution.rs`, `Makefile` `LINT_CRATES`, and
  `.github/workflows/rolling-release.yml`) do not change.
- The installer's own `--experimental` flag, which gates which lint crates are
  installed, is out of scope and does not change.
- Selector tokens stay case-sensitive. Invalid casing is a configuration error
  and is never normalized (`docs/whitaker-cli-design.md` §Rule identifiers and
  selection model).
- Code follows `AGENTS.md`: no file over 400 lines, a `//!` comment on every
  module, `///` on every function, no `expect` outside tests,
  `cap_std`/`camino` for filesystem access in tests, and en-GB-oxendict
  spelling.
- Verus proofs must not use `assume`, `admit`, `assume_specification`,
  `axiom`, `#[verifier::external]`, `#[verifier::external_body]`,
  `#[verifier::truncate]`, or any other trusted shortcut.

## Tolerances (exception triggers)

The ExecPlan file itself is excluded from every size count below.

- Scope: if implementation needs more than 18 non-ExecPlan files or more than
  1,400 net lines of code and other documentation, stop and escalate.
- Prerequisite drift: stop, record the gap in `Decision log`, set status
  `BLOCKED`, and re-plan if the merged 3.6.1 or 3.6.2 code differs from the
  assumed contract in `Interfaces and dependencies` by more than renaming. In
  particular, stop if:
  - resolution is not a pure function of catalogue, configuration, and
    experimental mode;
  - the catalogue cannot be replaced by a fixture in tests;
  - family selection matches string prefixes;
  - "no `select` supplied" cannot be told apart from `select = ["DEFAULT"]`;
  - there is no channel for non-fatal selection notices; or
  - exact codes are not fixed at three digits.
- Crate home: if 3.6.1 places codes and families where the Dylint suite and
  lint crates cannot depend on them (for example only inside `whitaker-cli`,
  which pulls in `ortho_config`), stop and escalate rather than adding a second
  copy of the band table (see D-5).
- Interface: if a public item of `whitaker-cli` outside the 3.6.1 and 3.6.2
  rule, selector, and resolution modules must change, stop and escalate.
- Dependencies: adding `googletest`, `pretty_assertions`, `insta`,
  `rstest-bdd`, and `rstest-bdd-macros` as dev-dependencies of the crate that
  hosts the catalogue is pre-approved, because all five are already workspace
  dependencies in the root `Cargo.toml`. Any other new dependency requires
  escalation.
- Proof: if a lemma in `O-CONS` still fails after three reformulations, or
  seems to need a trusted assumption, stop and escalate. Do not weaken a
  theorem statement to make it pass.
- Iterations: if a gate still fails after three fix attempts, stop and
  escalate.
- Ambiguity: if the CLI design, RFC 0011, and the Phase 17 amendment disagree
  about behaviour that a test would observe, stop and present the options.

## Risks

- Risk: neither 3.6.1 nor 3.6.2 is merged, and no branch implements either as
  of 2026-11-12. Severity: high. Likelihood: certain. Mitigation: D-1 orders
  this work after both, EP-M0 is a go/no-go gate, and
  `Interfaces and dependencies` lists what this item needs from them, so their
  authors can see it.
- Risk: a reserved code is renumbered before its rule ships, as the RFC 0012
  amendment already did when it moved `DOMAIN104` and `DOMAIN105` to
  `DOMAIN107` and `DOMAIN108`. Severity: medium. Likelihood: medium.
  Mitigation: D-3 rejects reserved codes until a rule ships, so no
  configuration can bind early to a code whose meaning may move.
- Risk: a `WorkspaceAggregate` rule (`DOMAIN9xx`) reaches the Dylint driver as
  a lint flag, and rustc reports an unknown lint, which fails under
  `-D warnings`. Severity: medium. Likelihood: medium, from 17.2.5 onwards.
  Mitigation: origin is recorded per rule now, the resolved selection exposes a
  lint-pass partition, and obligation `O-ORIGIN` tests that aggregate rules
  never appear in it.
- Risk: per-rule promotion (amendment 17.5.4 names `DOMAIN001`, `DOMAIN101`,
  and `DOMAIN106` first) collides with a family-wide opt-in policy. Severity:
  medium. Likelihood: high if the policy lived on the family. Mitigation: D-2
  makes explicit opt-in a per-rule property, checked by a catalogue test
  (`O-OPT-IN`) that promotion edits deliberately.
- Risk: the Verus model drifts from the production resolver. Severity: medium.
  Likelihood: medium. Mitigation: obligation `O-CONS-PROD` checks the same
  projection relation on production code exhaustively over a bounded scope, and
  `.github/workflows/proofs.yml` gains the catalogue crate's path, so a change
  to the resolver re-runs the proof.
- Risk: ADR numbers are allocated on unmerged branches. ADR 0013 and ADR 0014
  exist on other branches, the 9.1.1 plan claims ADR 0015, and `main` already
  has two `adr-0011-*` files. Severity: low. Likelihood: medium. Mitigation:
  EP-M0 re-fetches every remote branch and scans for ADR claims before ADR 0016
  is written.
- Risk: the first `DOMAIN` rule ships without its code being registered,
  because no roadmap item depends on 17.1.1. Severity: medium. Likelihood:
  medium. Mitigation: D-1 proposes making amendment items 17.2.1 and 17.5.1
  require 17.1.1. `Catalogue::validate` also rejects a `DOMAIN` rule outside a
  band.

## Progress

- [x] (2026-11-12) Renamed the branch to `17-1-1-reserve-domain-rule-family`.
- [x] (2026-11-12) Code and documentation reconnaissance (Wyvern team).
- [x] (2026-11-12) Prior-art review of Ruff preview-rule selection.
- [x] (2026-11-12) First draft of this ExecPlan.
- [x] (2026-11-12) Expert design review (structure and contracts; pre-mortem
  and alternatives; verification rigour). The verdict was "Revise", and this
  revision applies the findings (see the revision note).
- [ ] User approval of the plan and of decisions D-1 to D-4.
- [ ] EP-M0: prerequisite reconciliation, baseline snapshot, and gates.
- [ ] EP-M1: band model, rule origin, and `DOMAIN` family registration.
- [ ] EP-M2: Verus proof of conservative extension.
- [ ] EP-M3: selector integration, notices, and production ties.
- [ ] EP-M4: ADR, design and guide updates, and roadmap completion.

## Surprises & discoveries

- Observation: no rule-code, family, or selector-resolution code exists. The
  only selection surface is
  `Lint { select: Vec<String>, ignore: Vec<String>, experimental: bool }` in
  `cli/src/settings.rs`. Its `Default` sets `select = ["DEFAULT"]`, so an absent
  `select` cannot be told apart from an explicit one, and there is no
  `extend-select` field. Evidence: `cli/src/settings.rs`, the `Lint` struct and
  its `Default` impl. Impact: D-1. 3.6.2 must make `select` presence observable
  for the implicit `--experimental` rule to work at all.
- Observation: `Lint` uses `deny_unknown_fields`, so RFC 0011's example
  `extend-select = ["DOMAIN"]` fails to load today. The RFC uses a kebab-case
  key, while other `whitaker.toml` keys are snake_case
  (`allow_build_from_source`). Evidence: `cli/src/settings.rs`; RFC 0011
  §Configuration. Impact: this plan names no `extend-select` key. It uses
  whatever key 3.6.2 defines, and the users' guide shows only `select` until
  then.
- Observation: `LoadedConfiguration.notices` is already a `Vec<String>`, used
  for deprecation notices. Evidence: `cli/src/loader.rs`. Impact: the
  selection-notice channel is 3.6.2's to define, and this item adds
  `DOMAIN`-relevant cases to it rather than inventing a second channel.
- Observation: `--experimental` exists today only on `whitaker-installer`,
  where it gates which crates are installed. Evidence:
  `installer/src/resolution.rs`, `EXPERIMENTAL_LINT_CRATES`. Impact: the
  "experimental selector semantics" in this item belong to the `whitaker check`
  selection model. The installer is untouched.
- Observation: "report" means two things in the sources. RFC 0011 calls
  `DOMAIN900` a workspace report, whereas RFC 0012 and its amendment call
  `DOMAIN108` and `DOMAIN151` "report-only" rules even though they are lint
  passes in ordinary bands. Evidence: RFC 0012 amendment catalogue table.
  Impact: rule origin (`LintPass` versus `WorkspaceAggregate`) is kept separate
  from level.
- Observation: RFC 0012 §Rule catalogue says the bands let selectors "later
  distinguish `DOMAIN0` and `DOMAIN1` prefixes if Whitaker adopts numeric
  prefix selection". The CLI design defines no numeric-prefix selector, and it
  says only "uppercase ASCII followed by digits" for codes. Impact: D-4, and
  the three-digit requirement on 3.6.1.
- Observation: the Phase 17 amendment renumbers the integration items. The
  former 17.4.x are now 17.5.x, and guide updates belong to amendment 17.5.3.
  Evidence: `docs/roadmap-phase-17-domain-amendment.md` §17.5. Impact: EP-M4
  writes only what this item changes, and leaves the full family guide to
  17.5.3.
- Observation: the 3.5.2 plan on branch
  `3-5-2-whitaker-check-as-the-default-linting-path` claims ADR 0014, which
  branch
  `issue-529-separate-clippy-snapshot-policy-from-infrastructure-adapters` also
  uses. Impact: outside this item. It shows that the EP-M0 ADR scan matters.

## Decision log

### D-1: wait for 3.6.1 and 3.6.2, and wire 17.1.1 into the roadmap (awaiting approval)

- Decision: implement 17.1.1 only after both 3.6.1 and 3.6.2 merge. Change
  17.1.1's `Requires` clause in `docs/roadmap.md` and in the Phase 17 amendment
  from "3.6.1" to "3.6.1 and 3.6.2". Add 17.1.1 to the `Requires` clauses of
  amendment items 17.2.1 and 17.5.1 (and the matching `docs/roadmap.md` items
  17.2.1 and 17.4.1).
- Rationale: "experimental selector semantics", meaning precedence and the
  curated `--experimental` extension, are 3.6.2's (`docs/roadmap.md` 3.6.2).
  Waiting removes any proof about code that does not exist and any parked tests
  handed to another item. Nothing depends on 17.1.1, so the delay costs little,
  while the reverse edges ensure the first `DOMAIN` rule cannot ship
  unregistered.
- Alternatives: (a) Split 17.1.1, landing the band model now and selector
  integration later. Rejected, because the band model must attach to 3.6.1's
  family type, which does not exist. (b) The panel's strongest alternative:
  fold the `DOMAIN` row into 3.6.1 as data, and write the
  conservative-extension property once in 3.6.2. It would mean less text and no
  hand-off, at the cost of a larger 3.6.1 and 3.6.2 review and no standalone
  record of the `DOMAIN` decisions. Rejected, because 3.6.1 and 3.6.2 are
  planned separately and should stay reviewable. ADR 0016 keeps the decisions
  on record either way. If the user prefers (b), this plan becomes input to
  3.6.1 and 3.6.2.
- Affected upstream: `docs/roadmap.md` and
  `docs/roadmap-phase-17-domain-amendment.md`, edited with `mapsplice` in EP-M4.
- Approving authority: the user. Date/Author: 2026-11-12, planning agent.

### D-2: explicit opt-in is a per-rule property, checked for the whole family (awaiting approval)

- Decision: every `DOMAIN` rule is registered as experimental, not in
  `DEFAULT`, and not in the curated experimental set. This is held per rule,
  using whatever flags 3.6.1 and 3.6.2 define. A catalogue test (`O-OPT-IN`)
  asserts it for every `DOMAIN` rule. Promoting a rule (amendment 17.5.4) means
  changing its flags and the `O-OPT-IN` allow-list together, in one reviewed
  commit. There is no family-level experimental status or policy enum.
- Rationale: RFC 0011 always shows `DOMAIN` selected explicitly
  (`whitaker check --experimental --select DOMAIN`). Its members include
  `allow`-level and report-only rules whose evidence model is still being
  calibrated, so a user who turned on `--experimental` for the curated suite
  should not silently get every future `DOMAIN` rule. Promotion is per rule
  (amendment 17.5.4), so the property must be per rule as well. The proof shows
  this is exactly the premise that makes conservative extension hold for a bare
  `--experimental` (hypothesis H-NOTCURATED in `O-CONS`).
- Alternatives: (a) A family-level `ExplicitOptIn` policy. Rejected, because
  per-rule promotion would need exceptions bolted on, and the proof's premise
  would silently stop holding. (b) A user-level switch like Ruff's
  `explicit-preview-rules`. Rejected for now: it is a broader 3.6.2 feature,
  and it would not protect users who never set it.
- Affected upstream: `docs/whitaker-cli-design.md` §Rule identifiers and
  selection model gains one sentence: experimental rules are curated
  individually, and `DOMAIN` rules are not curated during their experimental
  phase.
- Approving authority: the user. Date/Author: 2026-11-12, planning agent.

### D-3: the family selector is valid; exact codes must name shipped rules (awaiting approval)

- Decision: `DOMAIN` is a valid selector even while it has no rules, and
  resolution reports `FamilyHasNoRules` when it expands to nothing. An exact
  `DOMAIN` code must name a shipped rule, as for every other family. A code
  that names no rule fails, with one of two messages:
  - `ReservedCode`: the code lies in a band, for example "`DOMAIN104` is
    reserved for type state spaces (`DOMAIN101`–`DOMAIN149`); no rule uses it
    yet."
  - `UnallocatedCode`: the code lies in a gap, for example "`DOMAIN100` is not
    allocated; `DOMAIN` codes are `001`–`099`, `101`–`149`, `150`–`179`,
    `180`–`199`, and `900`–`999`."
- Rationale: rejecting unshipped codes matches Ruff, which rejects unknown
  codes, and matches 3.6.1's behaviour for unbanded families. It also prevents
  early binding to codes the RFCs may still renumber. The family selector stays
  usable, which is what "reserve the family" needs. Unbanded families are
  untouched, because the reserved and gap messages exist only for banded
  families.
- Alternatives: accept reserved codes as inert selectors. Rejected after
  review. Combined with bands, it would weaken typo detection, and a reserved
  code could later bind silently to a renumbered rule.
- Approving authority: the user. Date/Author: 2026-11-12, planning agent.

### D-4: no numeric-prefix selectors (awaiting approval)

- Decision: `DOMAIN0`, `DOMAIN1`, `DOMAIN10`, and other partial numeric forms
  are invalid tokens. The message says that numeric-prefix selection is not
  supported, and suggests `DOMAIN` or an exact code. Codes are fixed at three
  digits, so `DOMAIN0001` is also invalid.
- Rationale: the CLI design defines five selector forms and no numeric
  prefixes, and RFC 0012 makes them conditional ("if Whitaker adopts").
  Accepting more tokens later is backward compatible, whereas withdrawing
  accepted tokens is not.
- Approving authority: the user. Date/Author: 2026-11-12, planning agent.

### D-5: banding is an explicit, opt-in property of a family

- Decision: a family's code allocation is either `Unbanded`, which behaves as
  3.6.1 defines, or `Banded(&'static [Band])`. A band has `start`, `end`, a
  stable `purpose_key: &'static str` (for example `"boundary-refinement"`), and
  an `origin: RuleOrigin`. A `const fn` validator runs over every banded table
  in a `const _: () = assert!(..)` item, so a malformed table fails the build.
  The band table is the single source of `DOMAIN` allocation. Selector
  messages, catalogue validation, and documentation read it, and nothing else
  may hard-code band boundaries. The table lives in the same crate as 3.6.1's
  rule codes, which must be one that the suite and lint crates can depend on.
  This matters because amendment 17.5.1 needs "stable rule metadata" there, and
  RFC 0011 wants codes rendered in diagnostics. If 3.6.1 chose otherwise, the
  crate-home tolerance escalates.
- Rationale: an explicit variant keeps unbanded families exactly as they were
  (the panel found that giving every family an implicit `001`–`999` band would
  change their error behaviour). A string purpose key, instead of a closed
  enum, stops each new banded family from editing a shared type, and keeps
  prose that may need translating out of the type. Compile-time validation
  means a built binary cannot contain an ill-formed allocation.
- Re-use policy: a family becomes banded only when an RFC allocates bands to
  it. Adding a banded family means adding one `Banded` table and its
  enumeration test, and nothing else.

### D-6: method selection for verification

- Decision: finite band logic is discharged on production code by exhaustive
  enumeration and compile-time evaluation. The unbounded conservative-extension
  property is proved in Verus. The proof is tied to production by a
  deterministic exhaustive small-scope test, with a proptest for longer
  selector lists. No Kani harness is written.
- Rationale: see `Verification plan`. The review panel was split on whether
  Verus is warranted. It is kept because the property is the contract later
  rules ship under, quantifies over unbounded catalogues and lists, and is
  stated as a projection whose `ALL` case carries real content (see
  `O-CONS-PROJ`). The sidecar is expected to be about 200 lines, comparable to
  `verus/whitaker_cli_dispatch.rs`.

### D-7: notices for selections that do nothing

- Decision: resolution reports two notices through 3.6.2's notice channel.
  `FamilyHasNoRules` is reported for a family selector that expands to no rules
  even in experimental mode. `RequiresExperimental` is reported for a selector
  that matches only experimental rules while experimental mode is off. If 3.6.2
  already reports the second for all experimental rules, reuse it. If it does
  not, add it generically, because the failure mode is not specific to `DOMAIN`.
- Rationale: pre-mortem scenario 1 (see `Artefacts and notes`): a CI job
  selects `DOMAIN` without `--experimental`, passes, and runs nothing for
  months. Ruff leaves a selected preview rule silently inactive in the same
  situation[^ruff-preview]; Whitaker deliberately reports it.

## Outcomes & retrospective

Not started. To be completed at each milestone and at the end.

## Context and orientation

The repository is a Cargo workspace. The parts that matter here:

- `cli/` is the `whitaker-cli` package, a library with no executable. It loads
  configuration through `ortho_config` from defaults, `whitaker.toml`, the
  environment, and the command line:
  - `cli/src/settings.rs` defines `Configuration` and the `Lint` table;
  - `cli/src/validated.rs` turns raw settings into `ValidatedSettings`;
  - `cli/src/error.rs` builds `OrthoError::Validation` values with
    `invalid(key, message)`;
  - `cli/src/loader.rs` provides `load_with_sources` and
    `LoadedConfiguration`.

  Integration tests live in `cli/tests/configuration/`, and unit tests sit
  beside their modules as `*_tests.rs` files.
- `src/` is the root `whitaker` binary from roadmap item 3.5.1. It does not
  yet depend on `whitaker-cli`. `whitaker check` arrives with 3.5.2.
- `suite/src/lints.rs` registers lint passes as
  `LintDescriptor { name, crate_name, pass }`. This item does not change it.
- `verus/` holds standalone Verus proof sidecars that model production code;
  for example, `verus/whitaker_cli_dispatch.rs` models `src/cli/dispatch.rs`.
  `scripts/run-verus.sh <group>` runs one group (`cli`, `clone-detector`,
  `decomposition`, or `string-literals`), and `make verus` runs them all.
  `.github/workflows/proofs.yml` runs `make verus` when files under its
  `paths:` list change.
- After 3.6.1 and 3.6.2 land, the rule catalogue, selector parser, and
  resolver will exist. Their paths are confirmed in EP-M0, and this section
  must then be updated. Below, `<rules>` stands for the module that holds them.

Documentation to read before starting:

- `docs/whitaker-cli-design.md`, §Rule identifiers and selection model and
  §Configuration model.
- `docs/rfcs/rfc-0011-weak-representations-at-domain-boundaries.md`, §Family
  contract and §Configuration.
- `docs/rfcs/rfc-0012-fragile-types-and-invalid-state-encodings.md`, §Rule
  catalogue, and
  `docs/rfcs/rfc-0012-amendment-state-machine-adt-and-protocol-coverage.md`,
  for the revised `DOMAIN101`–`DOMAIN108` and `DOMAIN151` catalogue.
- `docs/roadmap-phase-17-domain-amendment.md`, which governs Phase 17
  wording.
- `docs/adrs/adr-0008-ortho-config-cli-boundary.md` and
  `docs/adrs/adr-0012-whitaker-cli-boundary.md`.
- `docs/whitaker-dylint-suite-design.md`, for how experimental lints are
  registered and feature-gated.
- `docs/whitaker-clone-detector-design.md`, for the existing Verus sidecar
  conventions.
- `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rstest-bdd-users-guide.md`, `docs/rust-doctest-dry-guide.md`, and
  `docs/complexity-antipatterns-and-refactoring-strategies.md`.
- `docs/documentation-style-guide.md`, for the ADR format.
- The ExecPlans for 3.6.1 and 3.6.2, once they exist.

Skills to load:

- `rust-router` first, then `rust-types-and-apis` for the band and origin
  types;
- `rust-unit-testing` for rstest, googletest, pretty_assertions, and insta;
- `proptest` and `verus`;
- `arch-decision-records` for ADR 0016;
- `mapsplice` for the roadmap edits;
- the `codegraph-mcp` tools, to find the catalogue, parser, and resolver and
  their callers in EP-M0.

## Conformance basis

Upstream artefacts, at their revisions on `main` at commit `d9eb8a09`:

- `docs/rfcs/rfc-0011-weak-representations-at-domain-boundaries.md` (status
  Proposed, updated 2026-08-30).
- `docs/rfcs/rfc-0012-fragile-types-and-invalid-state-encodings.md` and its
  amendment (status Proposed, created 2026-08-30).
- `docs/roadmap-phase-17-domain-amendment.md`, normative for Phase 17.
- `docs/whitaker-cli-design.md` §Rule identifiers and selection model.
- Roadmap items 3.6.1 and 3.6.2. Their merged ExecPlans and code join this
  basis at EP-M0.

There is no Terms of Reference document for this work. The RFCs and the CLI
design play that role.

Traced requirements:

- `RFC11-ALLOC`: the five `DOMAIN` bands and their purposes (RFC 0011,
  Table 1).
- `RFC11-NODEFAULT`: "`DOMAIN` should not enter `DEFAULT` during the
  experimental phase."
- `RFC11-ALLEXP`: "`ALL` should include it only when experimental rules are
  enabled."
- `RFC11-NOGROUP`: "The selector does not depend on that compiler group
  existing."
- `AMEND-CATALOGUE`: the codes that RFC 0011 and the RFC 0012 amendment plan
  to use fall in their intended bands. These are `DOMAIN001`, `DOMAIN002`,
  `DOMAIN900`, `DOMAIN101`–`DOMAIN108`, and `DOMAIN151`.
- `AMEND-PROMOTE`: rules are promoted individually (amendment 17.5.4).
- `CLI-FORMS`: the five selector forms (`DEFAULT`, `ALL`, family, exact code,
  and lint name).
- `CLI-CASE`: tokens are case-sensitive, and invalid casing is a
  configuration error.
- `CLI-PREC`: the six-step selection precedence.
- `CLI-EXP-IMPLICIT`: `--experimental` without `select` extends the implicit
  default with experimental rules.
- `CLI-STABLE`: codes and selectors are stable and never translated.

Trace chains:

```plaintext
RFC11-ALLOC -> D-5 -> EP-M1 -> O-BAND-WF, O-BAND-TOTAL, O-CODE-FMT, O-ORIGIN
AMEND-CATALOGUE -> EP-M1 -> O-AMEND (planned_rfc_codes_fall_in_their_bands)
RFC11-NODEFAULT, RFC11-ALLEXP, AMEND-PROMOTE -> D-2 -> EP-M1 -> O-OPT-IN
RFC11-NODEFAULT, RFC11-ALLEXP, CLI-PREC, CLI-EXP-IMPLICIT -> D-2 -> EP-M2 -> O-CONS -> EP-M3 -> O-CONS-PROD
CLI-FORMS, CLI-CASE -> D-3, D-4 -> EP-M3 -> O-SEL-PARSE, O-PARSE-PRESERVE
D-7 -> EP-M3 -> O-NOTICE, domain_rule_family.feature
RFC11-NOGROUP -> Constraints -> EP-M3 (no lint group added)
CLI-STABLE -> EP-M3 -> selector error snapshots (insta)
```

## Verification plan

### Abstract model (proof sketch first)

A catalogue is a finite set of rules. Each rule `r` has:

- a family `r.f`;
- a number `r.n` in `0..=999`;
- a canonical name `r.name`;
- an origin;
- an `experimental` flag;
- an `in_default` flag; and
- a `curated` flag, meaning it belongs to the curated experimental set.

A selector is one of `Default`, `All`, `Family(f)`, `Code(f, n)`, or `Name(s)`.

Single-selector expansion `expand(C, s, e)`, where `e` is experimental mode:

- `expand(C, Default, e) = { r ∈ C | r.in_default }`;
- `expand(C, All, e) = { r ∈ C | ¬r.experimental ∨ e }`;
- `expand(C, Family(f), e) = { r ∈ C | r.f = f }`;
- `expand(C, Code(f, n), e) = { r ∈ C | r.f = f ∧ r.n = n }`, written as a
  comprehension so that it neither uses `choose` nor relies on codes being
  unique; and
- `expand(C, Name(s), e) = { r ∈ C | r.name = s }`.

For a sequence `l`, `expandAll(C, l, e)` is the union of the expansions of its
members.

A configuration is
`cfg = { sel: Option<Seq<Selector>>, ext: Seq<Selector>, ign: Seq<Selector> }`.
Here `sel` is the effective `select` after `ortho_config` has layered the file,
environment, and command line (axiom `A-MERGE`). `Some([])` counts as present:
it gives an empty baseline and no implicit extension. Resolution follows
`CLI-PREC` and `CLI-EXP-IMPLICIT`:

```plaintext
base(C, cfg, e) = match cfg.sel {
    Some(l) => expandAll(C, l, e),
    None    => expand(C, Default, e) ∪ (if e { { r ∈ C | r.curated } } else { ∅ }),
}
raw(C, cfg, e)     = (base ∪ expandAll(C, cfg.ext, e)) \ expandAll(C, cfg.ign, e)
resolve(C, cfg, e) = if e { raw } else { { r ∈ raw | ¬r.experimental } }
```

The `DOMAIN` extension `D` is a set of rules that satisfies these hypotheses:

- H-FAMILY: every rule in `D` has family `DOMAIN`, and no rule in `C` does.
- H-NAMES: `names(C) ∩ names(D) = ∅`.
- H-NODEFAULT: no rule in `D` is `in_default`.
- H-NOTCURATED: no rule in `D` is `curated`.
- H-EXPERIMENTAL: every rule in `D` is experimental.

A selector *references* `DOMAIN` if it is `Family(DOMAIN)` or
`Code(DOMAIN, n)`. A configuration is *DOMAIN-free* if none of its selectors
references `DOMAIN`. Every configuration that was valid before this item is
DOMAIN-free, because `DOMAIN` tokens did not parse before.

### Obligations

#### O-CONS: conservative extension (Verus)

This is the contractual property of the item. Registering `DOMAIN` never
changes the non-`DOMAIN` part of any existing configuration's result, and adds
`DOMAIN` rules only in the documented case. The proposed decomposition below is
a hypothesis about the proof's structure. Auxiliary lemmas about set
comprehension, sequence induction, and triggers are deferred until Verus
exposes the real verification conditions.

- `O-CONS-PROJ-EXPAND` (lemma): for every DOMAIN-free `s` and every `e`,
  `expand(C ∪ D, s, e) \ D == expand(C, s, e)`. Uses H-FAMILY and H-NAMES.
- `O-CONS-DISJ-EXPAND` (lemma): for every DOMAIN-free `s` and every `e`, if
  `s ≠ All` or `¬e`, then `expand(C ∪ D, s, e) ∩ D == ∅`. Uses H-FAMILY,
  H-NAMES, H-NODEFAULT, and, for the `All` case, H-EXPERIMENTAL.
- `O-CONS-UNION` (lemma): both of the above lift to `expandAll` over any
  finite sequence. Prove by induction on the sequence length.
- `O-CONS-PROJ` (theorem): for every DOMAIN-free `cfg` and every `e`,
  `resolve(C ∪ D, cfg, e) \ D == resolve(C, cfg, e)`. Uses H-FAMILY, H-NAMES,
  H-NODEFAULT, and H-NOTCURATED (the `None` branch). This holds even for `ALL`
  with experimental mode on: registration only ever *adds* `DOMAIN` rules and
  never changes which non-`DOMAIN` rules are selected or ignored, including when
  `ignore` contains `ALL`.
- `O-CONS-FOLD` (corollary): for every DOMAIN-free `cfg`, if `¬e`, or if
  `All` appears neither in `cfg.sel.unwrap_or([])` nor in `cfg.ext`, then
  `resolve(C ∪ D, cfg, e) == resolve(C, cfg, e)`. Uses `O-CONS-PROJ`,
  `O-CONS-DISJ-EXPAND`, and H-EXPERIMENTAL.
- `O-REACH` (witnesses): three concrete instances.
  - Inhabited antecedent: a non-empty `C` with a curated experimental rule, a
    non-empty `D`, `cfg = { sel: None, ext: [], ign: [] }`, and `e` true.
    The result is non-empty, contains the curated `C` rule, and contains no
    `D` rule. This shows the theorem's hypotheses are satisfiable in the
    implicit-experimental case.
  - The excluded `All` case is real: with `sel = Some([All])` and `e` true,
    `resolve(C ∪ D, ..)` contains every rule in `D`.
  - The family is reachable: with `sel = Some([Family(DOMAIN)])` and `e`
    true, the result is exactly `D`.
- Method: a Verus sidecar over the model, using `vstd` `Set` and `Seq` with
  extensional equality (`=~=`).
- Rationale: the property quantifies over unbounded catalogues and selector
  lists, so sampling cannot establish it, and it is the contract every later
  `DOMAIN` rule relies on. Proving it first also shaped the design:
  - resolution must be a pure function of catalogue, configuration, and
    mode, so the catalogue is a parameter;
  - the fold may combine expansions only by union and difference;
  - "no `select`" must be observable, which is a requirement on 3.6.2; and
  - D-2 must be a per-rule property (H-NOTCURATED).
- Not proved, deliberately: "with `e` false, no `DOMAIN` rule is enabled".
  It follows in one step from the final filter in `resolve`, so a proof would
  restate the definition. `O-CONS-PROD` checks it on production code.
- Artefact: `verus/whitaker_rule_family_extension.rs`, added to the `cli` and
  `all` groups in `scripts/run-verus.sh`. If it would exceed 400 lines, split
  it into `..._model.rs` and `..._lemmas.rs`.
- Evidence: `./scripts/run-verus.sh cli` and `make verus` report every
  function verified with no errors. In the red stage, before the lemma bodies
  exist, the statements fail with open postconditions; record that output.
- Non-vacuity and trusted base:
  - `O-REACH` shows the antecedents are satisfiable and that the excluded
    case changes the result.
  - Three negative controls, each a temporary edit that must make
    verification fail for the stated reason. Record each failure in
    `Artefacts and notes`.
    1. Drop H-FAMILY: `O-CONS-PROJ` fails.
    2. Drop H-NOTCURATED: the `None` branch of `O-CONS-PROJ` fails.
    3. Drop H-NODEFAULT: `O-CONS-DISJ-EXPAND` fails for `Default`.
  - Run the trusted-base search below and expect no matches:

    ```bash
    rg -n --pcre2 \
      '^(?!\s*//).*(assume|admit|assume_specification|axiom|verifier::(external|external_body|truncate))' \
      verus/whitaker_rule_family_extension*.rs
    ```

#### O-CONS-PROD: production tie for conservative extension

- Statement: the production resolver satisfies `O-CONS-PROJ` and
  `O-CONS-FOLD`.
- Method:
  - A deterministic, exhaustive small-scope test. `C` has four fixture rules:
    stable and in `DEFAULT`, stable and not in `DEFAULT`, curated
    experimental, and experimental in a second family. `D` is either empty
    or one fixture rule per `DOMAIN` band.
  - The test covers every DOMAIN-free configuration over a 12-token
    vocabulary: `sel` is `None` or a list of up to two tokens, `ext` and
    `ign` each hold up to one token, and `e` is either value. That is
    roughly 10⁵ cases, each run through the real parser and resolver, and
    each asserted against both properties.
  - A proptest with a weighted `prop_oneof!` token strategy covers lists of up
    to eight tokens.
- Rationale: the model abstracts away string parsing and real types, and this
  test closes that gap on the production code. A Kani harness is a poor fit.
  The resolver parses `String`s and builds sets, which needs unwind bounds and
  long solver runs. Over a finite vocabulary, bounded-exhaustive enumeration
  gives the same coverage on production code far more cheaply.
- Artefact: `<rules>/domain_family_props.rs`. The path is confirmed at EP-M0.
- Evidence: `make test`.
- Non-vacuity: the exhaustive test asserts that it ran at least one case for
  every selector form × `e` × `sel` presence combination. It also asserts at
  least one case where `D` changes the result (`ALL` with `e` true). As a
  seeded fault, make one `D` fixture rule `in_default` in the fixture builder;
  the test must fail. The fixed proptest weights make the selector mix explicit
  by construction.
- CI: add the catalogue crate's source path to the `paths:` list in
  `.github/workflows/proofs.yml`, so a resolver change re-runs `make verus`.

#### O-BAND-WF: band tables are well-formed

- Statement: in every `Banded` table, each band is non-empty
  (`start <= end`) and lies in `1..=999`, and the bands are strictly increasing
  and pairwise disjoint. Purpose keys are non-empty and unique within the
  family.
- Method: compile-time evaluation of `const fn bands_are_well_formed`, through
  `const _: () = assert!(..)` over every banded table. An rstest table also
  calls the same `const fn` at run time on malformed tables.
- Rationale: the shipped tables are constants, so const evaluation is a
  complete check of them. The run-time table shows that the validator can fail.
- Artefact: `<rules>/bands.rs` and `<rules>/bands_tests.rs`.
- Evidence: `make test`. A malformed shipped table fails `cargo build` with
  `evaluation of constant value failed`.
- Non-vacuity: run-time cases for an overlapping pair, an unsorted pair,
  `start > end`, a band starting at `0`, a band ending at `1000`, an empty
  purpose key, and a duplicate purpose key must each return `false`. As a
  seeded fault, temporarily start the second `DOMAIN` band at `99`; the build
  must fail. Record the transcript.

#### O-BAND-TOTAL: band classification is total and exact

- Statement: for every `n: u16`, `band_of(DOMAIN, n)` returns the unique band
  containing `n` if one exists, and `None` otherwise. `None` holds exactly for
  `0`, `100`, `200..=899`, and `1000..=65535`.
- Method: exhaustive enumeration of all 65,536 values, compared with an
  independent oracle that spells out RFC 0011 Table 1 as literal range
  patterns. An rstest table of named boundary cases (`0`, `1`, `99`, `100`,
  `101`, `149`, `150`, `179`, `180`, `199`, `200`, `899`, `900`, `999`, `1000`)
  gives readable failures.
- Rationale: the domain is finite and small, so enumeration is an exhaustive
  proof on the production function. Kani would add solver cost without adding
  coverage, and Verus would prove a model instead of the code.
- Artefact: `<rules>/domain_family_tests.rs`.
- Evidence: `make test`.
- Non-vacuity: the test asserts per-band hit counts of 99, 49, 30, 20, and
  100, and 65,238 misses (1 + 1 + 700 + 64,536). This stops a copied oracle
  from agreeing with a wrong implementation. Moving any boundary by one changes
  a count.

#### O-CODE-FMT: string-level round trip

- Statement: for every `n` in `0..=999`, parsing the displayed code
  `DOMAIN` + `{n:03}` gives `Code(DOMAIN, n)` when a rule with that number is
  in the catalogue. Otherwise it gives `ReservedCode` when `band_of` is `Some`,
  and `UnallocatedCode` when it is `None`.
- Method: exhaustive enumeration over the production catalogue and over a
  fixture catalogue with one rule per band.
- Rationale: users meet codes as strings, so the check runs at the string
  level, including zero padding.
- Artefact: `<rules>/domain_family_tests.rs`.
- Evidence: `make test`.
- Non-vacuity: the fixture catalogue makes all three outcomes occur. Count
  the outcomes and assert the expected totals.

#### O-ORIGIN: band, origin, and uniqueness

- Statement: for every catalogue that passes `Catalogue::validate`:
  - every rule of a banded family lies in a band;
  - its origin equals the band's origin; and
  - no two rules share a code or a name.

  The resolved selection's lint-pass partition never contains a
  `WorkspaceAggregate` rule.
- Method: an rstest table of fixture catalogues, each expecting a specific
  `CatalogueError`:
  - a `LintPass` rule at `DOMAIN950`;
  - a `WorkspaceAggregate` rule at `DOMAIN001`;
  - a rule at `DOMAIN100`;
  - two rules at `DOMAIN001`; and
  - two rules with the same name.

  A test asserts that the production catalogue validates. A further test checks
  the partition on a fixture catalogue containing both origins.
- Rationale: these are finite, explicit partitions, so parameterized tests are
  proportionate.
- Artefact: catalogue validation tests from 3.6.1, extended.
- Evidence: `make test`.
- Non-vacuity: every error variant has a witness, and the valid fixture
  catalogue reaches every `DOMAIN` band and both origins.

#### O-OPT-IN: every `DOMAIN` rule is explicit opt-in

- Statement: every `DOMAIN` rule in the production catalogue is experimental,
  not in `DEFAULT`, and not curated, unless its code is on the promotion
  allow-list. The allow-list is empty until amendment 17.5.4.
- Method: a unit test over the production catalogue. A negative control runs
  the same check on a fixture catalogue with a `DOMAIN` rule in `DEFAULT`, and
  the check must reject it.
- Rationale: this is the production-side guard for the Verus premises
  H-NODEFAULT, H-NOTCURATED, and H-EXPERIMENTAL.
- Non-vacuity: today the production catalogue has no `DOMAIN` rules, so the
  production run is vacuous. The plan accepts that and says so. The negative
  control shows the check works, and the check becomes active when 17.2.1 ships
  `DOMAIN001`.

#### O-AMEND: planned RFC codes fit their bands

- Statement: each planned code classifies into the intended band and origin:
  - `DOMAIN001` and `DOMAIN002`: boundary refinement, `LintPass`;
  - `DOMAIN101` to `DOMAIN108`: state space, `LintPass`;
  - `DOMAIN151`: local protocol, `LintPass`;
  - `DOMAIN900`: grouped report, `WorkspaceAggregate`.
- Method: an rstest table named `planned_rfc_codes_fall_in_their_bands`.
- Rationale: this is a conformance check against the upstream documents, not
  an invariant. It catches a band edit that would strand a planned rule.

#### O-SEL-PARSE: selector grammar for the family

- Statement:
  - `DOMAIN` parses as `Family(DOMAIN)`.
  - `DOMAIN104` and `DOMAIN999` give `ReservedCode` against the production
    catalogue.
  - `DOMAIN000`, `DOMAIN100`, `DOMAIN200`, and `DOMAIN899` give
    `UnallocatedCode`.
  - `DOMAIN0`, `DOMAIN1`, `DOMAIN10`, and `DOMAIN0001` give the
    numeric-prefix `InvalidToken` message (D-4).
  - `Domain` and `DOMAIN01a` give `InvalidToken`.
  - `domain` has the shape of a lint name under `CLI-CASE`, so it gives
    3.6.1's unknown-lint-name error.
  - `DO` and `DOMAINX` give `UnknownFamily`.
  - `DOC` selects no `DOMAIN` rule.
- Method: an rstest table of token to expected result, using googletest
  matchers on the error variant. A proptest generates pairs of families where
  one is a strict string prefix of the other, and checks that the shorter
  family's selector selects no rule of the longer one.
- Rationale: the table pins the specified tokens. The property guards the
  family-identity rule in general.
- Non-vacuity: the proptest builds the longer family as the shorter one plus
  a non-empty uppercase suffix, so every case is a prefix pair. As a seeded
  fault, replace family equality with `starts_with`; the proptest must find a
  counterexample.

#### O-PARSE-PRESERVE: unbanded families are unchanged

- Statement: every token in 3.6.1's vocabulary parses to the same result, or
  the same error variant, as at the EP-M0 baseline. The vocabulary is every
  family, every code `XXX000`–`XXX999` of every unbanded family, every lint
  name, `DEFAULT`, and `ALL`.
- Method: an insta snapshot of the parse results, generated at EP-M0 before
  any change and kept unchanged afterwards.
- Non-vacuity: the snapshot includes both successes and errors for every
  unbanded family.

#### O-NOTICE: notices are emitted exactly when a selection does nothing

- Statement:
  - `FamilyHasNoRules` is emitted exactly for selectors `s` where
    `expand(C, s, true)` is empty.
  - `RequiresExperimental` is emitted exactly for selectors `s` where `e` is
    false, `expand(C, s, true)` is non-empty, and every rule in it is
    experimental.
- Method: checked inside the exhaustive small-scope test of `O-CONS-PROD`,
  plus named rstest cases and the behaviour scenarios below.
- Non-vacuity: the fixture catalogues make both notices occur and both fail
  to occur. Counts are asserted.

#### Error-message snapshots

Every new `SelectorError` and `CatalogueError` `Display` string gets an insta
snapshot. That includes `ReservedCode`, `UnallocatedCode`, the numeric-prefix
`InvalidToken` message, `CodeOutsideBands`, and `OriginMismatch`. These strings
are the user-visible contract that neither the model nor the properties
describe.

### Axioms

- `A-MERGE`: `ortho_config` merges sources in the documented order (defaults,
  files, environment, then command line). The model's `cfg.sel`, `cfg.ext`, and
  `cfg.ign` are the post-merge values. The existing `cli/tests/configuration`
  suite covers this; it is not re-verified here.
- `A-VSTD`: Verus and the `vstd` set and sequence axioms are sound. This is
  the standard trusted base.
- `A-PARSE`: 3.6.1's selector parser is the only path from configuration
  strings to selectors. EP-M0 confirms this with `codegraph_get_callers`.

### Relationship between proof and tests

The Verus proof establishes conservative extension for all catalogues and
configurations, but for a model. The exhaustive small-scope test and the
proptest check the same relation on the real code. Enumeration and const
evaluation cover the finite band logic directly on production code. The rstest
tables, insta snapshots, and behaviour scenarios pin the user-visible contract
(accepted tokens, error text, and notices), which neither the model nor the
properties describes. Every obligation keeps an executable test, even where a
proof exists.

## Plan of work

Every milestone follows the same pattern. Stage A reconciles with no code
change. Stage B writes the failing tests, or the open proof obligations, and
records the expected failure. Stage C makes the smallest production change that
turns them green. Stage D refactors, documents, runs every gate, updates this
plan, and commits.

## Milestones and plateaus

### EP-M0: prerequisite reconciliation

- Outcome: a written mapping from the assumed interface to the merged 3.6.1
  and 3.6.2 code, the baseline parse snapshot, and green baseline gates.
- Steps:
  1. Confirm that 3.6.1 and 3.6.2 are ticked in `docs/roadmap.md` and merged
     on `main`.
  2. Rebase this branch onto `main`.
  3. Read both ExecPlans and the catalogue, parser, and resolver modules. Use
     `codegraph_symbol_search` and `codegraph_get_callers` to find every
     caller.
  4. Check each prerequisite-drift and crate-home trigger in `Tolerances`.
  5. Capture the `O-PARSE-PRESERVE` snapshot.
  6. Re-fetch all remote branches and scan for ADR number claims.
  7. Run `make check-fmt`, `make lint`, and `make test`.
- Acceptance: `Context and orientation` and `Interfaces and dependencies` name
  real paths. The baseline snapshot is committed. The baseline gates pass, with
  logs under `/tmp`.
- Conformance check: both prerequisites are present, and there is no drift
  beyond renaming; otherwise set status `BLOCKED`.
- Recovery: apart from the snapshot commit, this milestone is read-only and
  repeatable.
- Compatibility decision: none.

### EP-M1: band model, rule origin, and `DOMAIN` family registration

- Outcome:
  - families carry a `CodeAllocation`, and banded tables are validated at
    compile time;
  - rules carry a `RuleOrigin`;
  - `DOMAIN` is registered with its five bands and no rules;
  - catalogue validation enforces band membership, origin, and uniqueness;
    and
  - the `O-OPT-IN` guard is in place.
- Requirements: `RFC11-ALLOC`, `AMEND-CATALOGUE`, `AMEND-PROMOTE`, D-2, and
  D-5.
- Red: add tests for `O-BAND-WF`, `O-BAND-TOTAL`, `O-ORIGIN`, `O-OPT-IN`, and
  `O-AMEND`. They fail to compile because the types do not exist.
- Green: add `CodeAllocation`, `Band`, `RuleOrigin`,
  `bands_are_well_formed`, `band_of`, the `DOMAIN` family entry, the validation
  rules, and the lint-pass partition accessor.
- Acceptance: the new tests pass, and `O-PARSE-PRESERVE` is still unchanged.
  `make check-fmt`, `make lint`, and `make test` pass.
- Conformance check:
  - the bands match RFC 0011 Table 1 exactly;
  - unbanded families are untouched;
  - no lint crate or lint group has been added.
- Recovery: revert the milestone commit.
- Remaining gaps: selectors do not accept `DOMAIN` tokens yet.
- Compatibility decision: none.

### EP-M2: Verus proof of conservative extension

- Outcome: the sidecar proves `O-CONS-PROJ-EXPAND`, `O-CONS-DISJ-EXPAND`,
  `O-CONS-UNION`, `O-CONS-PROJ`, `O-CONS-FOLD`, and `O-REACH`, and runs in the
  `cli` group.
- Requirements: `RFC11-NODEFAULT`, `RFC11-ALLEXP`, `CLI-PREC`,
  `CLI-EXP-IMPLICIT`, and D-2.
- Red: write the model and the lemma statements with empty bodies, run
  `./scripts/run-verus.sh cli`, and record the open postconditions.
- Green: write proof bodies until the group verifies. Then run the three
  negative controls and record their failures.
- Acceptance:
  - `./scripts/run-verus.sh cli` and `make verus` pass;
  - the trusted-base search finds nothing;
  - `proofs.yml` includes the catalogue crate's path; and
  - `make check-fmt`, `make lint`, and `make test` pass.
- Conformance check: the model's `base`, `raw`, and `resolve` match the merged
  3.6.2 resolver step by step, with a comment on each step that cites the Rust
  function it mirrors. If the proof shows D-2 is insufficient, stop and
  escalate.
- Recovery: the sidecar is standalone, so reverting it touches nothing else.
- Compatibility decision: none.

### EP-M3: selector integration, notices, and production ties

- Outcome:
  - the parser accepts `DOMAIN`;
  - it rejects reserved and gap codes with distinct errors, and rejects
    numeric prefixes;
  - resolution emits `FamilyHasNoRules` and `RequiresExperimental`; and
  - the production property tests pass.
- Requirements: `CLI-FORMS`, `CLI-CASE`, `CLI-STABLE`, `RFC11-NOGROUP`, D-3,
  D-4, and D-7.
- Red: write the `O-SEL-PARSE` table and proptest, `O-CODE-FMT`,
  `O-CONS-PROD`, `O-NOTICE`, the error snapshots, the behaviour feature below,
  and the configuration-loading end-to-end tests.
- Green: extend the parser and the resolver's notices. Map the new
  `SelectorError` variants to `OrthoError::Validation` through
  `cli::error::invalid`, using the configuration key that 3.6.1 and 3.6.2 use
  for the list in which the token appeared.
- End-to-end: write real `whitaker.toml` files into a temporary directory with
  `cap_std`. Load them with `load_with_sources`, validate them, and resolve
  them against the production catalogue. Assert on the selection, the notices,
  and the error text. This is the outermost boundary available until 3.5.2
  connects configuration to the `whitaker` binary; binary-level checks belong
  to 3.5.2.
- Acceptance: all new tests pass, the snapshots are reviewed, and
  `O-PARSE-PRESERVE` is unchanged. `make check-fmt`, `make lint`, and
  `make test` pass.
- Conformance check: codes are not translated, casing errors are unchanged,
  and the existing 3.6.1 and 3.6.2 tests are untouched and green.
- Recovery: revert the milestone commit; EP-M1 and EP-M2 stand alone.
- Compatibility decision: none.

Behaviour specification, in `<crate>/tests/features/domain_rule_family.feature`
with steps in `<crate>/tests/domain_rule_family_bdd.rs`. Scenarios that need
`DOMAIN` rules use a fixture catalogue at the resolution layer. Scenarios that
use the production catalogue go through configuration loading. `given` and
`when` steps return `Result` and never assert; all assertions are in `then`
steps.

```gherkin
Feature: DOMAIN rule family selection
  The DOMAIN family is reserved, experimental, and explicitly opted into.

  Scenario: The default selection excludes the DOMAIN family
    Given a fixture catalogue with rule DOMAIN001
    And no lint selection
    When the selection is resolved with experimental mode off
    Then no DOMAIN rule is enabled

  Scenario: A bare experimental run does not enable DOMAIN
    Given a fixture catalogue with rule DOMAIN001
    And no lint selection
    When the selection is resolved with experimental mode on
    Then no DOMAIN rule is enabled

  Scenario: Selecting DOMAIN without experimental mode explains itself
    Given a fixture catalogue with rule DOMAIN001
    And the lint selection "DOMAIN"
    When the selection is resolved with experimental mode off
    Then no DOMAIN rule is enabled
    And a notice reports that "DOMAIN" requires experimental mode

  Scenario: Selecting DOMAIN with experimental mode enables the family
    Given a fixture catalogue with rules DOMAIN001 and DOMAIN101
    And the lint selection "DOMAIN"
    When the selection is resolved with experimental mode on
    Then exactly DOMAIN001 and DOMAIN101 are enabled

  Scenario Outline: ALL includes DOMAIN only in experimental mode
    Given a fixture catalogue with rule DOMAIN001
    And the lint selection "ALL"
    When the selection is resolved with experimental mode <mode>
    Then DOMAIN001 is <state>

    Examples:
      | mode | state    |
      | off  | disabled |
      | on   | enabled  |

  Scenario: The empty DOMAIN family loads with a notice
    Given a whitaker.toml selecting "DEFAULT" and "DOMAIN" with experimental mode on
    When the configuration is loaded and resolved
    Then the configuration is valid
    And a notice reports that "DOMAIN" has no implemented rules

  Scenario: A reserved code is rejected as reserved
    Given a whitaker.toml selecting "DOMAIN104"
    When the configuration is loaded and validated
    Then validation fails for the select key
    And the message says "DOMAIN104" is reserved for type state spaces

  Scenario: A code in a gap is rejected as unallocated
    Given a whitaker.toml selecting "DOMAIN100"
    When the configuration is loaded and validated
    Then validation fails for the select key
    And the message lists the DOMAIN bands
```

### EP-M4: ADR, design and guide updates, and roadmap completion

- Outcome: the decisions are recorded, the family is discoverable, and the
  roadmap item is ticked.
- Steps:
  1. Write
     `docs/adrs/adr-0016-banded-rule-code-allocation-and-explicit-opt-in-rules.md`
     in the documentation style guide's ADR format, covering D-2, D-3, D-4,
     D-5, and D-7. The number is re-confirmed in EP-M0.
  2. Update `docs/whitaker-cli-design.md` §Rule identifiers and selection
     model. Add:
     - the `DOMAIN` row and banded allocation;
     - the three-digit code rule;
     - the reserved and unallocated code errors;
     - the notices;
     - explicit opt-in as a per-rule property; and
     - the conservative-extension contract, citing ADR 0016 and the Verus
       sidecar.
  3. Add a short note to `docs/users-guide.md` under rule selection. It should
     say that the `DOMAIN` selector exists, that it has no rules yet, how to
     opt in, and what the two notices and two errors mean. The full family
     guide belongs to amendment 17.5.3.
  4. Add a section to `docs/developers-guide.md` covering:
     - how to allocate a code within a band;
     - the band table's re-use policy (D-5);
     - how to promote a rule through `O-OPT-IN`; and
     - how to run the `cli` Verus group.
  5. Cross-reference ADR 0016 from RFC 0011 §Rule-code allocation, and list it
     in `docs/contents.md`.
  6. Apply D-1's `Requires` edits with `mapsplice`. Mark 17.1.1 done in
     `docs/roadmap.md` and in `docs/roadmap-phase-17-domain-amendment.md`.
- Acceptance: `make fmt`, `make markdownlint`, `make nixie`, `make spelling`,
  `make check-fmt`, `make lint`, and `make test` all pass.
- Conformance check: every trace chain resolves to a passing artefact, and
  every discovery has been reconciled into the CLI design and the ADR.
- Recovery: these are documentation-only commits, and each can be reverted on
  its own.
- Compatibility decision: none.

## Concrete steps

Run everything from the worktree root, and capture gate output with `tee`:

```bash
BR=$(git branch --show-current)
make check-fmt 2>&1 | tee /tmp/check-fmt-whitaker-$BR.out
make lint 2>&1 | tee /tmp/lint-whitaker-$BR.out
make test 2>&1 | tee /tmp/test-whitaker-$BR.out
./scripts/run-verus.sh cli 2>&1 | tee /tmp/verus-cli-whitaker-$BR.out
make verus 2>&1 | tee /tmp/verus-whitaker-$BR.out
```

Run the gates one after another, never in parallel. Rust builds queue for
compiler slots through `build-limits-rustc`, and a waiting build is not hung.

While iterating, run focused tests. Substitute the package name confirmed at
EP-M0:

```bash
cargo nextest run -p <package> domain_family 2>&1 | tee /tmp/test-domain-whitaker-$BR.out
cargo insta review -p <package>
```

A successful Verus run ends with:

```plaintext
verification results:: N verified, 0 errors
```

Commit after each milestone once the gates are green. Use an imperative subject
that names the roadmap item, for example
`Register the DOMAIN family bands (17.1.1)`.

## Validation and acceptance

The item is done when all of the following are observable:

- Loading a `whitaker.toml` with `select = ["DEFAULT", "DOMAIN"]` and
  `experimental = true` succeeds, and reports that `DOMAIN` has no implemented
  rules.
- With a fixture `DOMAIN001`, selecting `DOMAIN` while experimental mode is
  off enables nothing, and reports that experimental mode is required.
- Loading `select = ["DOMAIN104"]` fails with a "reserved" message, and
  loading `select = ["DOMAIN100"]` fails with a message that lists the five
  bands.
- `O-PARSE-PRESERVE` is unchanged, and every existing 3.6.1 and 3.6.2 test
  passes unmodified.
- `./scripts/run-verus.sh cli` verifies the sidecar, and the trusted-base
  search finds nothing.
- `make check-fmt`, `make lint`, `make test`, and `make verus` all pass.

Quality criteria:

- Tests: all new rstest, proptest, insta, and rstest-bdd tests pass.
- Verification: `O-BAND-WF`, `O-BAND-TOTAL`, `O-CODE-FMT`, and `O-ORIGIN` are
  discharged on production code. `O-CONS` is discharged in Verus, and
  `O-CONS-PROD` passes.
- Lint: `make lint` is clean, including Whitaker's own Dylint suite.

## Idempotence and recovery

Every step can be repeated. Tests write only to temporary directories. Each
milestone is one or more commits that can be reverted independently. EP-M3
builds on EP-M1, and the Verus sidecar stands alone.

## Artefacts and notes

To be filled in during implementation:

- the red and green transcripts;
- the negative-control failures for `O-BAND-WF`, `O-CONS`, `O-CONS-PROD`, and
  `O-SEL-PARSE`; and
- the reviewed insta snapshots.

Expert-review record (2026-11-12). The panel examined the first draft through
three lenses:

- structure and contracts;
- pre-mortem and alternatives; and
- verification rigour.

Its pre-mortem scenarios, and where this revision answers each:

1. A CI job selects `DOMAIN` without `--experimental` and silently runs
   nothing. Answered by D-7 (`RequiresExperimental`).
2. A `DOMAIN9xx` aggregate reaches the Dylint driver as an unknown lint.
   Answered by `RuleOrigin` and `O-ORIGIN`.
3. Per-rule promotion collides with a family-level policy. Answered by D-2
   (per-rule, with `O-OPT-IN`).

## Interfaces and dependencies

Assumed contract from 3.6.1 and 3.6.2. Confirm it in EP-M0, and rename here as
needed:

- `RuleCode { family: FamilyId, number: u16 }`. It displays as the family
  followed by the zero-padded three-digit number, and parses only with exactly
  three digits.
- `FamilyId`, a validated identifier of uppercase ASCII letters.
- `Selector`, with variants for `DEFAULT`, `ALL`, family, code, and lint name,
  and a fallible parse against a catalogue.
- `Catalogue`, a value with a production constructor and a fixture builder
  for tests. It carries, per rule: code, canonical name, `experimental`,
  `in_default`, and membership of the curated experimental set.
- A pure resolver from (catalogue, merged configuration, experimental mode)
  to a resolved selection plus notices. The merged configuration distinguishes
  an absent `select` from a present one.
- All of the above live in a crate that the Dylint suite and lint crates can
  depend on.

Additions in this item:

```rust
/// How a family allocates rule-code numbers.
pub enum CodeAllocation { Unbanded, Banded(&'static [Band]) }

/// A closed interval of rule-code numbers within a banded family.
pub struct Band { pub start: u16, pub end: u16, pub purpose_key: &'static str, pub origin: RuleOrigin }

/// Where a rule's findings come from; independent of its level.
pub enum RuleOrigin { LintPass, WorkspaceAggregate }

/// Return `true` when bands are non-empty, in `1..=999`, sorted, disjoint, and uniquely keyed.
pub const fn bands_are_well_formed(bands: &[Band]) -> bool;

/// Return the band of `allocation` containing `number`, if the family is banded and one does.
pub fn band_of(allocation: &CodeAllocation, number: u16) -> Option<&Band>;
```

- `SelectorError` gains `ReservedCode { code, band }` and
  `UnallocatedCode { family, number, bands }`. The numeric-prefix case reuses
  `InvalidToken`, with a dedicated message.
- `CatalogueError` gains `CodeOutsideBands` and `OriginMismatch`.
- The notice type gains `FamilyHasNoRules { selector }` and, unless 3.6.2
  already provides it, `RequiresExperimental { selector }`.
- The resolved selection gains a lint-pass partition accessor, which excludes
  `WorkspaceAggregate` rules.

Dependencies: no new external crates. The catalogue crate gains the
dev-dependencies `googletest`, `pretty_assertions`, `insta`, `rstest-bdd`, and
`rstest-bdd-macros` where it lacks them; all five are already workspace
dependencies.

## Revision note

2026-11-12, after expert design review. Changes:

- D-1 now requires 3.6.2 as well as 3.6.1, and adds reverse `Requires`
  edges. All parked tests and skipped scenarios for 3.6.2 are removed.
- D-2 moves explicit opt-in from a family policy to a per-rule property with
  an `O-OPT-IN` guard, because promotion is per rule.
- D-3 now rejects reserved, unshipped codes with a "reserved" message instead
  of accepting them as inert, which avoids early binding to codes that may be
  renumbered. The Ruff citation is corrected: Ruff rejects unknown codes.
- D-5 replaces implicit `General` bands with an explicit
  `CodeAllocation::{Unbanded, Banded}`, so existing families are unchanged, and
  adds the crate-home requirement.
- `RuleKind::Report` becomes `RuleOrigin::WorkspaceAggregate`, to separate
  origin from the RFCs' "report-only" level.
- D-7 adds the `RequiresExperimental` and `FamilyHasNoRules` notices.
- The verification model now has `sel: Option<..>`, and its lemmas are
  restated as a projection theorem, which covers `ignore` containing `ALL`.
  Named-rule modelling and hypotheses are explicit, the witnesses inhabit the
  antecedent, and there are three negative controls.
- `O-CONS-PROD` becomes a deterministic exhaustive small-scope test plus a
  weighted proptest. `O-CODE-FMT`, `O-ORIGIN`, `O-OPT-IN`, `O-PARSE-PRESERVE`,
  and `O-NOTICE` are added, and every error string is snapshotted.
- The behaviour feature uses a `Scenario Outline` for `ALL`, asserts notices,
  and drops the skip mechanism.
- Guide work is narrowed to what this item changes. Amendment 17.5.3 owns the
  full family guide.

Effect on remaining work: the plan cannot start until 3.6.2 merges as well as
3.6.1. Once it starts, it no longer leaves residual obligations for other items.

[^ruff-preview]: Ruff documentation, "Preview", section "Using rules that are
    in preview": preview rules are implemented rules that stay inactive until
    preview mode is enabled, even when selected by code, prefix, or `ALL`.
    Whitaker's `RequiresExperimental` notice makes the equivalent situation
    visible. <https://docs.astral.sh/ruff/preview/>
