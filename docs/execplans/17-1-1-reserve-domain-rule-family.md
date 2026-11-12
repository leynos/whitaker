# Reserve the `DOMAIN` rule family and integrate it with experimental selection

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (awaiting approval; blocked on roadmap item 3.6.1 and on
decisions D-1 to D-4)

Roadmap item: 17.1.1 in `docs/roadmap.md`, governed by the normative
`docs/roadmap-phase-17-domain-amendment.md`.

## Purpose / big picture

Whitaker is growing a family of domain-modelling lints, described in RFC 0011
("Validate not, only parse") and RFC 0012 with its amendment. Every rule in
that family will carry a stable code beginning with `DOMAIN`, and users will
opt in with `whitaker check --experimental --select DOMAIN` or the equivalent
`whitaker.toml` settings. Before the first rule exists, the family itself must
be registered so that:

1. the five numeric bands for its rule codes are fixed in code, not only in
   prose, so that no later rule can take a code from the wrong band or from a
   gap between bands;
2. the selector `DOMAIN`, and exact codes in the reserved bands, are accepted by
   configuration validation, while gap codes such as `DOMAIN100` or `DOMAIN500`
   are rejected with a message that names the valid bands; and
3. registering the family is invisible to every existing user. No
   configuration that was valid before the change selects a different set of
   rules afterwards, unless it asks for experimental rules through `ALL` with
   experimental mode on.

Point 3 is the contract this item is really about, and the plan proves it
formally (see `Verification plan`, obligation `O-CONS`).

After this change, a user who writes the following `whitaker.toml` gets a
configuration that loads and validates. It enables no rules yet, and the
resolution reports a notice that the `DOMAIN` family has no implemented rules:

```toml
[lint]
experimental = true
select = ["DEFAULT", "DOMAIN"]
```

A user who writes `select = ["DOMAIN100"]` gets a configuration error naming
the reserved bands. A user who never mentions `DOMAIN` sees no change at all.

Terms used throughout:

- *Rule code*: a stable, uppercase identifier such as `PAN001`. It is a family
  prefix of uppercase ASCII letters followed by exactly three decimal digits.
- *Family*: the letter prefix of a rule code, such as `PAN` or `DOMAIN`. A
  family selector selects every rule whose family is equal to the selector. It
  never matches on string prefixes, so `DO` selects nothing and `DOC` does not
  select `DOMAIN001`.
- *Band*: a closed numeric interval of rule-code numbers within one family,
  with a declared purpose and rule kind. `DOMAIN` has five bands.
- *Gap*: a number in `000` to `999` that lies in no band of its family. For
  `DOMAIN` the gaps are `000`, `100`, and `200` to `899`.
- *Rule kind*: `Diagnostic` for an ordinary Dylint lint, or `Report` for a
  workspace-level grouped report produced by `whitaker check` aggregation
  rather than by a lint pass. Only the `DOMAIN900` to `DOMAIN999` band holds
  `Report` rules.
- *Catalogue*: the in-memory registry of families and rules that selector
  resolution consults. Roadmap item 3.6.1 introduces it.
- *Experimental mode*: the boolean merged from `[lint].experimental`,
  `WHITAKER_LINT_EXPERIMENTAL`, and `--experimental`.
- *Explicit opt-in family*: a family whose rules are excluded from `DEFAULT`
  and from the implicit `--experimental` extension, and are reached only
  through a selector that names the family or one of its codes, or through
  `ALL` with experimental mode on (decision D-2).

## Constraints

- Roadmap item 3.6.1 (stable rule codes, family selectors, and `DEFAULT`/`ALL`
  semantics) must be merged to `main` before any production code in this plan
  is written. This plan builds on 3.6.1's catalogue and selector types; it must
  not invent a parallel registry. EP-M0 enforces this.
- Rule codes, family names, band boundaries, and selector spellings are stable,
  language-neutral identifiers. They must never be translated
  (`docs/whitaker-cli-design.md`, "Command names, rule codes, manifest keys,
  and JSON fields are stable and are never translated").
- The five bands are exactly those in RFC 0011 §Rule-code allocation, Table 1,
  as restated by the Phase 17 amendment: `001`–`099`, `101`–`149`, `150`–`179`,
  `180`–`199`, and `900`–`999`. Changing a boundary requires an RFC change, not
  an implementation choice.
- `DOMAIN` must not enter `DEFAULT`. `ALL` includes `DOMAIN` rules only when
  experimental mode is on (RFC 0011 §Display name and selector).
- No lint crate, no `whitaker::domain` compiler lint group, and no Fluent
  message is added. The selector does not depend on a compiler lint group
  existing (RFC 0011 §Category, not monolith). Lint crates arrive in 17.2 and
  17.3.
- The three hand-maintained lint-crate lists (`installer/src/resolution.rs`,
  `Makefile` `LINT_CRATES`, `.github/workflows/rolling-release.yml`) must not
  change, because no lint crate is added.
- Selector tokens remain case-sensitive. Invalid casing is a configuration
  error, never normalized (`docs/whitaker-cli-design.md` §Rule identifiers and
  selection model).
- Code follows `AGENTS.md`: no file over 400 lines, `//!` module comments,
  `///` on every function, no `expect` outside tests, `cap_std`/`camino` for
  filesystem access in tests, and en-GB-oxendict spelling in comments and
  documentation.
- Verus proofs must not use `assume`, `admit`, `#[verifier::external_body]`,
  or any other trusted shortcut.

## Tolerances (exception triggers)

The ExecPlan file itself is excluded from every size count below.

- Scope: if implementation needs more than 16 non-ExecPlan files or more than
  1,400 net lines of code and other documentation, stop and escalate.
- Prerequisite drift: if 3.6.1's merged catalogue or selector API differs from
  the assumed contract in `Interfaces and dependencies` by more than renaming
  (for example, no pure expansion function, a global static catalogue that
  cannot be injected in tests, or family matching by string prefix), stop,
  record the gap in `Decision log`, set status `BLOCKED`, and re-plan EP-M1 to
  EP-M3.
- Interface: if a public item of `whitaker-cli` other than the 3.6.1 catalogue
  and selector types must change, stop and escalate.
- Dependencies: adding workspace dev-dependencies that already appear in the
  root `Cargo.toml` (`googletest`, `pretty_assertions`, `insta`, `rstest-bdd`,
  `rstest-bdd-macros`) to `cli/Cargo.toml` is pre-approved. Any other new
  dependency requires escalation.
- Proof: if a Verus lemma in `O-CONS` still fails after three reformulations,
  or appears to need a trusted assumption, stop and escalate. Do not weaken a
  theorem statement.
- Iterations: if a gate still fails after three fix attempts, stop and
  escalate.
- Ambiguity: if the CLI design, RFC 0011, and the Phase 17 amendment disagree
  about behaviour a test would observe, stop and present the options.

## Risks

- Risk: roadmap item 3.6.1 is not merged, and no branch implements it as of
  2026-11-12. Severity: high. Likelihood: certain. Mitigation: decision D-1
  orders this work strictly after 3.6.1, and EP-M0 is a go/no-go gate. The plan
  records the interface it assumes so that 3.6.1's author can see what this
  item needs.
- Risk: 3.6.2 (selector precedence and curated `--experimental`) may not have
  landed when this item is implemented, so the implicit `--experimental`
  extension may not exist in code yet. Severity: medium. Likelihood: high.
  Mitigation: this item records the family's opt-in policy as data
  (`ExperimentalPolicy::ExplicitOptIn`), proves the precedence-level lemma
  against the design specification, and leaves a named, tracked production test
  for 3.6.2 to enable (see `O-CONS-FOLD`).
- Risk: the 3.6.1 catalogue may be a global `static` that tests cannot
  replace, which blocks fixture `DOMAIN` rules. Severity: medium. Likelihood:
  medium. Mitigation: the tolerance above escalates. The plan asks for
  resolution functions that take the catalogue as a parameter, which the
  `O-CONS` proof needs anyway.
- Risk: ADR numbers are allocated on unmerged branches. ADR 0013 and ADR 0014
  exist on other branches, and the 9.1.1 plan claims ADR 0015. Severity: low.
  Likelihood: medium. Mitigation: EP-M0 re-fetches every remote branch and
  scans for `adr-00NN` claims before writing ADR 0016.
- Risk: accepting reserved but unimplemented codes (decision D-3) could hide a
  typo such as `DOMAIN010` meant as `DOMAIN001`. Severity: low. Likelihood:
  medium. Mitigation: resolution returns a `SelectionNotice` naming every
  selector that matched no implemented rule, and gap codes are hard errors.
- Risk: the Verus model drifts from the Rust implementation. Severity: medium.
  Likelihood: medium. Mitigation: a production-level property test
  (`O-CONS-PROD`) checks the same conservative-extension relation on the real
  resolution code over generated catalogues and selector lists. The model file
  names the mirrored Rust items.

## Progress

- [x] (2026-11-12) Renamed the branch to `17-1-1-reserve-domain-rule-family`.
- [x] (2026-11-12) Reconnaissance of code and documentation (Wyvern team).
- [x] (2026-11-12) Prior-art review of Ruff preview-rule selection.
- [x] (2026-11-12) First draft of this ExecPlan.
- [ ] Expert design review and revision.
- [ ] User approval of the plan and of decisions D-1 to D-4.
- [ ] EP-M0: prerequisite reconciliation and baseline gates.
- [ ] EP-M1: band model and `DOMAIN` family registration.
- [ ] EP-M2: Verus proof of conservative extension.
- [ ] EP-M3: selector integration, behaviour tests, and the production
  property test.
- [ ] EP-M4: documentation, ADR, and roadmap completion.

## Surprises & discoveries

- Observation: no rule-code, family, or selector-resolution code exists. The
  only selection surface is
  `cli::settings::Lint { select: Vec<String>,
  ignore: Vec<String>, experimental: bool }`,
  with untyped strings and no `extend-select` field. Evidence:
  `cli/src/settings.rs` (the `Lint` struct and its `Default` impl). A search for
  `RuleCode`, `rule_code`, `Selector`, and `DOMAIN[0-9]` in `*.rs` found
  nothing. Impact: 3.6.1 must land first (D-1).
- Observation: the `--experimental` flag today exists only on
  `whitaker-installer`, where it gates which lint crates are installed
  (`installer/src/resolution.rs`, `EXPERIMENTAL_LINT_CRATES`). Evidence: the
  code survey. Impact: "experimental selector semantics" in this item refers to
  the future `whitaker check` selection model, not the installer flag. The
  installer is untouched.
- Observation: RFC 0012 says the bands let "selectors later distinguish
  `DOMAIN0` and `DOMAIN1` prefixes if Whitaker adopts numeric prefix
  selection". The CLI design does not define numeric-prefix selectors. Evidence:
  `docs/rfcs/rfc-0012-fragile-types-and-invalid-state-encodings.md` §Rule-code
  allocation. Impact: decision D-4.
- Observation: RFC 0011 configures per-rule tables as `[lint.<rule>]`, while
  the CLI design and `cli/src/settings.rs` use top-level `[<rule>]` tables.
  Evidence: RFC 0011 §Configuration; `cli/src/settings.rs`. Impact: none for
  this item, which adds no per-rule table. Recorded for 17.2.1.
- Observation: the 3.5.2 ExecPlan on branch
  `3-5-2-whitaker-check-as-the-default-linting-path` claims ADR 0014, which the
  branch
  `issue-529-separate-clippy-snapshot-policy-from-infrastructure-adapters` also
  uses. Evidence: `git ls-tree` over fetched remote branches. Impact: outside
  this item. It is noted so the ADR scan in EP-M0 is taken seriously.

## Decision log

### D-1: order this item strictly after 3.6.1 (awaiting approval)

- Decision: implement 17.1.1 only after 3.6.1 merges. Build the family on
  3.6.1's catalogue and selector types. Do not split this item, and do not
  pre-build a rule-code registry here.
- Rationale: the item's value is entirely in selection. Without 3.6.1 there
  is nothing to integrate with, and a registry built here would pre-empt
  3.6.1's design and invite rework. The item is small, so splitting it (the
  approach taken by the 3.5.2 plan) would buy little.
- Alternatives: (a) split 17.1.1, landing the band model now in a neutral
  module and the selector integration later. Rejected because the band model
  must attach to 3.6.1's `Family` type, which does not exist. (b) Absorb 3.6.1
  into this item. Rejected as scope creep across a separately planned roadmap
  item.
- Approving authority: the user. Date/Author: 2026-11-12, planning agent.

### D-2: `DOMAIN` is an explicit opt-in experimental family (awaiting approval)

- Decision: `DOMAIN` rules are experimental and are excluded from `DEFAULT`
  and from the implicit `--experimental` extension. They are enabled only by a
  selector that names `DOMAIN` or a `DOMAIN` code, or by `ALL`, and in every
  case only when experimental mode is on. The policy is a property of the family
  (`ExperimentalPolicy::ExplicitOptIn`), not of each rule.
- Rationale: the CLI design says `--experimental` without `select` "extends
  the implicit default selection with experimental rules" to keep "the curated
  suite immediately useful", and roadmap 3.6.2 calls this behaviour "curated".
  RFC 0011 always shows `DOMAIN` selected explicitly
  (`whitaker check --experimental --select DOMAIN`). Its rules include
  `allow`-level and report-only members whose evidence model is still being
  calibrated. This decision is also exactly the premise under which the
  conservative-extension lemma `O-CONS-FOLD` holds for the implicit
  `--experimental` case. Without it, adding a `DOMAIN` rule would silently
  change the result of a bare `whitaker check --experimental`.
- Alternatives: include `DOMAIN` in the implicit extension. Rejected because a
  user who adopted `--experimental` for the curated suite would get every future
  `DOMAIN` rule without asking.
- Affected upstream: `docs/whitaker-cli-design.md` §Rule identifiers and
  selection model gains a sentence defining explicit opt-in families. ADR 0016
  records the decision.
- Approving authority: the user. Date/Author: 2026-11-12, planning agent.

### D-3: reserved codes are valid selectors; gap codes are errors (awaiting approval)

- Decision: an exact code inside a reserved band (for example `DOMAIN001`
  before `discarded_parsed_value` exists) is a valid selector that resolves to
  no rules and produces a `SelectionNotice::NoImplementedRules`. The family
  selector `DOMAIN` behaves the same way while the family has no rules. An
  exact code in a gap (`DOMAIN000`, `DOMAIN100`, `DOMAIN200` to `DOMAIN899`) is
  a configuration error, `SelectorError::UnallocatedCode`, whose message lists
  the family's bands.
- Rationale: a configuration written against a newer Whitaker should load on
  an older one without editing, which matches the stability goal for codes. Gap
  codes can never become valid without an RFC change, so rejecting them is safe
  and catches typos. Ruff's treatment of preview rules (selected but inert
  until preview is enabled) is precedent for selected-but-inert
  selectors[^ruff-preview].
- Alternatives: reject every code that names no implemented rule. Rejected
  because it makes the family selector itself an error until 17.2.1 lands,
  which contradicts "reserve the family".
- Approving authority: the user. Date/Author: 2026-11-12, planning agent.

### D-4: numeric-prefix selectors are out of scope (awaiting approval)

- Decision: `DOMAIN0`, `DOMAIN1`, and similar partial numeric prefixes are
  rejected as invalid selectors with `SelectorError::InvalidToken`. Band
  selection may be added later by an RFC; accepting more tokens later is
  backward compatible, while removing accepted tokens would not be.
- Rationale: the CLI design defines only five selector forms, and RFC 0012
  phrases numeric prefixes conditionally ("if Whitaker adopts").
- Approving authority: the user. Date/Author: 2026-11-12, planning agent.

### D-5: bands are a generic family capability, validated at compile time

- Decision: every family carries a band table. Families registered by 3.6.1
  get a single `001`–`999` `Diagnostic` band, which leaves their behaviour
  unchanged. `DOMAIN` declares its five bands. A `const fn` validator checks
  every family table during compilation, through a `const _: () = assert!(..)`
  item, so a malformed table fails the build rather than a test.
- Rationale: banding is a policy about codes, not about `DOMAIN`. A single
  mechanism avoids a `DOMAIN` special case in selector parsing. Compile-time
  validation makes an ill-formed allocation unrepresentable in a built binary.
- Re-use policy: the band table is the only place a family's numeric
  allocation is defined. Selector parsing, catalogue validation, and
  documentation snapshots read it; nothing else may hard-code band boundaries.
  New families declare bands only when an RFC allocates them.

### D-6: no proof for the finite parts; Verus for conservative extension

- Decision: band classification and table well-formedness are discharged by
  exhaustive enumeration and compile-time evaluation, not by Kani or Verus. The
  unbounded conservative-extension property is proved in Verus over a model,
  and tied to production code by a property test.
- Rationale: see `Verification plan`.

## Outcomes & retrospective

Not started. To be completed at each milestone and at the end.

## Context and orientation

The repository is a Cargo workspace. The parts that matter here:

- `cli/` is the `whitaker-cli` package, a library with no executable. It loads
  configuration through `ortho_config` from defaults, `whitaker.toml`, the
  environment, and the command line. `cli/src/settings.rs` defines
  `Configuration` and the `Lint` table. `cli/src/validated.rs` turns raw
  settings into `ValidatedSettings`. `cli/src/error.rs` builds
  `OrthoError::Validation` values with `invalid(key, message)`. Integration
  tests live in `cli/tests/configuration/`, and unit tests sit beside their
  modules as `*_tests.rs` files.
- `src/` is the root `whitaker` binary from roadmap item 3.5.1. It does not
  depend on `whitaker-cli` yet. `whitaker check` arrives with 3.5.2.
- `suite/src/lints.rs` registers lint passes with
  `LintDescriptor { name, crate_name, pass }`. It has no rule codes, and this
  item does not change it.
- `verus/` holds standalone Verus proof sidecars that mirror production code.
  For example, `verus/whitaker_cli_dispatch.rs` mirrors `src/cli/dispatch.rs`.
  `scripts/run-verus.sh <group>` runs a group (`cli`, `clone-detector`, and
  others), and `make verus` runs all groups. `.github/workflows/proofs.yml` runs
  `make verus` on changes under `verus/**`.
- After 3.6.1 lands, a rule catalogue and selector parser are expected in
  `whitaker-cli`, most likely under `cli/src/rules/`. The exact paths are
  confirmed in EP-M0, and this section must then be updated with them.

Documentation to read before starting:

- `docs/whitaker-cli-design.md` §Rule identifiers and selection model and
  §Configuration model.
- `docs/rfcs/rfc-0011-weak-representations-at-domain-boundaries.md` §Family
  contract (§Display name and selector, §Rule-code allocation, §Category, not
  monolith) and §Configuration.
- `docs/rfcs/rfc-0012-amendment-state-machine-adt-and-protocol-coverage.md`,
  for the revised `DOMAIN101` to `DOMAIN108` and `DOMAIN151` catalogue.
- `docs/roadmap-phase-17-domain-amendment.md`, which governs Phase 17 wording.
- `docs/adrs/adr-0008-ortho-config-cli-boundary.md` and
  `docs/adrs/adr-0012-whitaker-cli-boundary.md`.
- `docs/whitaker-dylint-suite-design.md`, for how experimental lints are
  registered and feature-gated.
- `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rstest-bdd-users-guide.md`, `docs/rust-doctest-dry-guide.md`, and
  `docs/complexity-antipatterns-and-refactoring-strategies.md`.
- `docs/whitaker-clone-detector-design.md`, for the established Verus and Kani
  sidecar conventions.
- `docs/documentation-style-guide.md`, for the ADR template.

Skills to load: `rust-router` first, then `rust-types-and-apis` for the band
and family types, `rust-unit-testing` for rstest, googletest, and insta,
`proptest`, `verus`, and `arch-decision-records` for ADR 0016. Use the
`codegraph-mcp` tools to find 3.6.1's catalogue callers in EP-M0.

## Conformance basis

Upstream artefacts at the revisions on `main` at commit `d9eb8a09`:

- `docs/rfcs/rfc-0011-weak-representations-at-domain-boundaries.md` (status
  Proposed, updated 2026-08-30).
- `docs/rfcs/rfc-0012-amendment-state-machine-adt-and-protocol-coverage.md`
  (status Proposed, created 2026-08-30).
- `docs/roadmap-phase-17-domain-amendment.md` (normative for Phase 17).
- `docs/whitaker-cli-design.md` §Rule identifiers and selection model.
- Roadmap item 3.6.1. Its merged ExecPlan and code will join this basis at
  EP-M0.

There is no Terms of Reference document for this work. The RFCs and the CLI
design play that role.

Traced requirements:

- `RFC11-ALLOC`: the five `DOMAIN` bands and their purposes (RFC 0011 Table 1).
- `RFC11-NODEFAULT`: "`DOMAIN` should not enter `DEFAULT` during the
  experimental phase."
- `RFC11-ALLEXP`: "`ALL` should include it only when experimental rules are
  enabled."
- `RFC11-NOGROUP`: "The selector does not depend on that compiler group
  existing."
- `AMEND-CATALOGUE`: the amendment's codes `DOMAIN101` to `DOMAIN108` and
  `DOMAIN151`, plus RFC 0011's `DOMAIN001`, `DOMAIN002`, and `DOMAIN900`, all
  fall in their intended bands.
- `CLI-FORMS`: the five selector forms (`DEFAULT`, `ALL`, family, exact code,
  lint name).
- `CLI-CASE`: case-sensitive tokens; invalid casing is a configuration error.
- `CLI-PREC`: the six-step selection precedence.
- `CLI-EXP-IMPLICIT`: `--experimental` without `select` extends the implicit
  default with the curated experimental set.
- `CLI-STABLE`: codes and selectors are stable and never translated.

Trace chains:

```plaintext
RFC11-ALLOC -> D-5 -> EP-M1 -> O-BAND-TOTAL, O-BAND-WF, O-KIND (cli rules domain tests)
AMEND-CATALOGUE -> EP-M1 -> planned_rfc_codes_fall_in_their_bands
RFC11-NODEFAULT, RFC11-ALLEXP -> D-2 -> EP-M2 -> O-CONS (Verus) -> EP-M3 -> O-CONS-PROD (proptest)
CLI-EXP-IMPLICIT -> D-2 -> EP-M2 -> O-CONS-FOLD (Verus); production tie deferred to 3.6.2
CLI-FORMS, CLI-CASE -> D-3, D-4 -> EP-M3 -> O-SEL-PARSE (rstest table, BDD)
RFC11-NOGROUP -> Constraints -> EP-M3 (no lint group added)
CLI-STABLE -> EP-M1 -> allocation ledger snapshot (insta)
```

## Verification plan

### Abstract model

Model a catalogue as a finite set of rules. Each rule has a family `f`, a number
`n` in `0..=999`, a kind (`Diagnostic` or `Report`), an `experimental` flag, an
`in_default` flag, and a canonical lint name. Each family has a band table, an
experimental status, and an `ExperimentalPolicy` (`Curated` or
`ExplicitOptIn`). A selector is one of `Default`, `All`, `Family(f)`,
`Code(f, n)`, or `Name(s)`; a lint name denotes the same rule as its code, so
the model treats `Name` as `Code`.

Single-selector expansion, `expand(C, s, e)`, where `e` is experimental mode:

- `expand(C, Default, e)` is the set of rules in `C` with `in_default`.
- `expand(C, All, e)` is every rule in `C` that is not experimental, plus
  every experimental rule when `e` holds.
- `expand(C, Family(f), e)` is every rule in `C` whose family equals `f`.
- `expand(C, Code(f, n), e)` is the rule in `C` with family `f` and number `n`,
  or the empty set.

The precedence fold, `resolve(C, cfg, e)`, follows `CLI-PREC`. Start from
`expand(C, Default, e)`. Replace it with the union of the configuration
`select` list, then with the union of the CLI `--select` list, when present.
When neither `select` is present and `e` holds, add the curated experimental
set: the experimental rules whose family policy is `Curated`. Add the union of
`extend-select`, remove the union of `ignore`, and finally remove every
experimental rule when `e` is false.

The `DOMAIN` extension `D` is a set of rules whose family is `DOMAIN`, all
experimental, none `in_default`, with family policy `ExplicitOptIn`. `DOMAIN`
is not a family of any rule in `C`. A selector *references* `DOMAIN` when it is
`Family(DOMAIN)` or `Code(DOMAIN, n)`. A configuration is *DOMAIN-free* when no
selector in any of its lists references `DOMAIN`. Every configuration that was
valid against `C` alone is DOMAIN-free, because `DOMAIN` selectors did not
parse before this item.

### Obligations

#### O-BAND-WF: band tables are well-formed

- Statement: for every family, bands are non-empty (`start <= end`), lie in
  `1..=999`, are strictly increasing, and are pairwise disjoint.
- Method: compile-time evaluation of a `const fn bands_are_well_formed` over
  every family table, through `const _: () = assert!(..)`, plus an rstest table
  calling the same `const fn` at run time on malformed tables.
- Rationale: the tables are compile-time constants, so a const check is a
  complete proof for the shipped tables. The run-time table shows the validator
  can fail.
- Artefact: the band module created in EP-M1 (expected
  `cli/src/rules/bands.rs`) and its `bands_tests.rs`.
- Evidence: `make test`. A malformed shipped table fails `cargo build` with an
  `evaluation of constant value failed` error.
- Non-vacuity: run-time rstest cases for an overlapping pair, an unsorted
  pair, an empty band (`start > end`), a band starting at `0`, and a band
  ending at `1000` must each return `false`. As a seeded fault, temporarily
  change `DOMAIN` band two to start at `99`; the build must fail. Record the
  transcript in `Artefacts and notes`.

#### O-BAND-TOTAL: band classification is total and exact

- Statement: for every `n: u16`, `band_of(DOMAIN, n)` returns the unique band
  whose interval contains `n` when one exists, and `None` otherwise. In
  particular, `None` holds exactly for `0`, `100`, `200..=899`, and
  `1000..=65535`.
- Method: exhaustive enumeration over all 65,536 `u16` values, compared with
  an independent oracle that spells out RFC 0011 Table 1 as literal range
  patterns in the test. Add an rstest table of named boundary cases (`0`, `1`,
  `99`, `100`, `101`, `149`, `150`, `179`, `180`, `199`, `200`, `899`, `900`,
  `999`, `1000`) for readable failure messages.
- Rationale: the input domain is finite and small, so enumeration is an
  exhaustive proof that checks the production function directly. Kani would add
  solver cost without adding coverage, and Verus would prove a model rather
  than the code.
- Artefact: `domain_family_tests.rs` beside the family registration.
- Evidence: `make test`.
- Non-vacuity: the enumeration counts hits per band and asserts the counts
  are 99, 49, 30, 20, and 100, and 65,238 misses. A mutation that moves any
  boundary by one changes a count and fails.

#### O-KIND: kind follows band, and codes are unique

- Statement: in any catalogue that passes `Catalogue::validate`, every rule's
  number lies in a band of its family, the rule's kind equals that band's kind,
  and no two rules share a code or a canonical name.
- Method: an rstest table of fixture catalogues (a `Diagnostic` rule at
  `DOMAIN950`, a `Report` rule at `DOMAIN001`, a rule at `DOMAIN100`, two rules
  at `DOMAIN001`), each expecting the matching `CatalogueError` variant. A unit
  test asserts that the production catalogue validates.
- Rationale: finite, explicit partitions; parameterized tests are
  proportionate.
- Artefact: catalogue validation tests from 3.6.1, extended in EP-M1.
- Evidence: `make test`.
- Non-vacuity: each error variant has a witness. The valid-catalogue case
  includes one fixture rule per `DOMAIN` band, so every band is reachable.

#### O-AMEND: planned RFC codes fit their bands

- Statement: `DOMAIN001`, `DOMAIN002` (boundary refinement, `Diagnostic`),
  `DOMAIN101` to `DOMAIN108` (state space, `Diagnostic`), `DOMAIN151` (local
  protocol, `Diagnostic`), and `DOMAIN900` (grouped report, `Report`) each
  classify into the band and kind the RFCs intend.
- Method: an rstest table.
- Rationale: this is a conformance check against upstream documents, not an
  invariant. Its value is catching a band edit that would strand a planned rule.
- Artefact: `domain_family_tests.rs`, test
  `planned_rfc_codes_fall_in_their_bands`.
- Evidence: `make test`.

#### O-SEL-PARSE: selector grammar for the family

- Statement: `DOMAIN` parses as `Family(DOMAIN)`. `DOMAIN001`, `DOMAIN149`,
  `DOMAIN150`, `DOMAIN199`, `DOMAIN900`, and `DOMAIN999` parse as
  `Code(DOMAIN, n)`. `DOMAIN000`, `DOMAIN100`, `DOMAIN200`, and `DOMAIN899` are
  `UnallocatedCode`. `DOMAIN0`, `DOMAIN1`, `DOMAIN0001`, `domain`, `Domain`, and
  `DOMAIN01a` are `InvalidToken`. `DO` and `DOMAINX` are `UnknownFamily`.
  `DOC` does not select `DOMAIN001`.
- Method: an rstest table of tokens to expected parse results, and a proptest
  that for any pair of distinct generated families `f` and `g`, where `g` is a
  string prefix of `f`, `Family(g)` selects no rule of family `f`.
- Rationale: the table pins the specified tokens. The property guards the
  family-identity rule against a prefix-matching regression in general, not
  only for `DOC` and `DOMAIN`.
- Artefact: selector tests from 3.6.1, extended in EP-M3.
- Evidence: `make test`.
- Non-vacuity: the proptest strategy builds `f` from `g` plus a non-empty
  uppercase suffix, so every case is a prefix pair. Proptest's own reporting
  shows the number of cases run. A seeded fault that replaces equality with
  `starts_with` must produce a counterexample.

#### O-CONS: conservative extension (Verus)

This is the contractual property of the item: registering the `DOMAIN` family
is invisible to every configuration that was valid before it.

- `O-CONS-EXPAND` (lemma): for any catalogue `C`, extension `D` as defined in
  the abstract model, selector `s` that does not reference `DOMAIN`, and mode
  `e`, if `s` is not `All` or `e` is false, then
  `expand(C ∪ D, s, e) == expand(C, s, e)`.
- `O-CONS-UNION` (lemma): the same holds for the union of the expansions of
  any finite sequence of selectors meeting the same conditions. Prove by
  induction on the sequence length.
- `O-CONS-FOLD` (theorem): for any DOMAIN-free configuration `cfg`, and any
  `e` such that `e` is false or no `All` appears in an effective `select` or
  `extend-select` list, `resolve(C ∪ D, cfg, e) == resolve(C, cfg, e)`. This
  includes the implicit `--experimental` case, and it depends on `D`'s family
  policy being `ExplicitOptIn` (D-2).
- `O-REACH` (witness lemma): there exist `C`, a non-empty `D`, and a
  configuration selecting `Family(DOMAIN)` with `e` true for which
  `resolve(C ∪ D, cfg, e)` contains every rule of `D`. A second witness with
  `All` and `e` true shows that excluded case really changes the result.
- Method: a Verus sidecar proof over the abstract model, using `vstd` `Set`
  and `Seq` with extensional set equality.
- Rationale: the property quantifies over unbounded catalogues and selector
  lists, so sampling cannot establish it. It is the contract later `DOMAIN`
  rules rely on to ship without disturbing existing users. Writing the proof
  first also shapes the API. Resolution must be a pure function of catalogue,
  configuration, and mode, so the catalogue must be a parameter. The fold must
  combine expansions only by union and difference. The curated experimental set
  must be defined by family policy, which is exactly D-2.
- Artefact: `verus/whitaker_cli_rule_family_extension.rs`, added to the `cli`
  and `all` groups in `scripts/run-verus.sh`.
- Evidence: `./scripts/run-verus.sh cli` reports every function verified and
  no errors, and `make verus` passes. Before the lemma bodies exist, the
  statements fail with open postconditions. That is the red stage.
- Non-vacuity and trusted base: `O-REACH` shows the antecedents are inhabited
  and that the excluded `All` case is not vacuous. As a negative control,
  temporarily set one `D` rule's `in_default` to true in the model's `D`
  predicate; `O-CONS-FOLD` must fail to verify. Separately, change `D`'s policy
  to `Curated`; the implicit `--experimental` case must fail. Record both
  failures in `Artefacts and notes`. Run
  `rg -n 'assume|admit|external_body|external' verus/whitaker_cli_rule_family_extension.rs`
  and expect no matches.
- Lemma decomposition is a hypothesis. Auxiliary lemmas about set
  comprehension, sequence folds, and triggers are left until Verus exposes the
  actual verification conditions.
- Deliberately not proved: "with `e` false, no `DOMAIN` rule is enabled". It
  follows in one step from the final filter in `resolve`, so a proof would
  restate the definition. A proptest covers it (`O-CONS-PROD`).

#### O-CONS-PROD: production tie for conservative extension

- Statement: for generated catalogues `C` (zero to six rules over the existing
  families), generated `DOMAIN` fixture extensions `D` (zero to four rules,
  covering all five bands), generated DOMAIN-free selector lists from `C`'s
  vocabulary, and generated `e`, the production expansion over `C ∪ D` equals
  the expansion over `C`, except for `All` with `e` true. With `e` false, the
  production result contains no `DOMAIN` rule.
- Method: proptest, with `prop_assume!` avoided in favour of constructive
  strategies.
- Rationale: this links the Verus model to the real code, whose selection
  types and string parsing the model abstracts.
- Artefact: `domain_family_props.rs` (path confirmed at EP-M0).
- Evidence: `make test`.
- Non-vacuity: classify cases with `prop_assert!`-adjacent counters that
  record how often each selector form appears. Fail the test if any form has
  fewer than 5 % of cases. As a seeded fault, mark one `D` rule `in_default` in
  the fixture builder; the property must fail.
- Residual gap: if 3.6.2's precedence fold has not landed, the production tie
  covers expansion and the union lemma only. `O-CONS-FOLD` is then proved
  against the design specification, and a tracked test,
  `domain_family_is_invisible_to_resolution`, is added in EP-M3 behind
  `#[ignore = "requires roadmap 3.6.2 resolve()"]`. 3.6.2's plan must remove
  the ignore. Record this hand-off in `docs/roadmap.md` notes for 3.6.2 if
  needed.

### Axioms

- `ortho_config` merges sources in the documented order (defaults, files,
  environment, CLI). Verified by the existing `cli/tests/configuration` suite,
  not re-verified here.
- Verus and `vstd` set and sequence axioms are sound. This is the standard
  trusted base.
- 3.6.1's selector parser is the only path from configuration strings to
  selectors. EP-M0 confirms this with `codegraph_get_callers`.

### Relationship between proof and tests

The Verus proof establishes the semantic property over all catalogues and
configurations, for a model. The property test checks the same relation on the
real code over generated inputs. Exhaustive enumeration and const evaluation
cover the finite band logic directly on production code. The rstest tables and
the behaviour scenarios pin the user-visible contract (accepted tokens, error
messages, and notices) that neither the model nor the property describes. Every
obligation keeps an executable test even where a proof exists.

## Plan of work

### Stage A: reconcile (EP-M0)

No code changes. Confirm prerequisites, map assumed names to real ones, and
record baselines.

### Stage B: red

For each milestone, write the failing tests or open proof obligations first,
run them, and record the failure in `Artefacts and notes`.

### Stage C: green

Make the smallest production change that turns each red test green, keeping
verification in step.

### Stage D: refactor and document

Tidy, document, run every gate, and update this plan.

## Milestones and plateaus

### EP-M0: prerequisite reconciliation

- Outcome: a written mapping from the assumed interface (see
  `Interfaces and dependencies`) to 3.6.1's merged code, and green baseline
  gates.
- Steps: confirm 3.6.1 is ticked in `docs/roadmap.md` and merged on `main`.
  Rebase this branch on `main`. Read 3.6.1's ExecPlan and catalogue module. Use
  `codegraph_symbol_search` and `codegraph_get_callers` to find the catalogue,
  selector parser, and expansion function, and every caller. Check whether
  3.6.2 has merged. Re-fetch all remote branches and scan for ADR number
  claims. Run `make check-fmt`, `make lint`, and `make test`.
- Requirements: none discharged. This is a gate.
- Acceptance: `Context and orientation` and `Interfaces and dependencies` name
  real paths. Baseline gates pass, with logs under `/tmp`.
- Conformance check: prerequisite present; no drift beyond renaming, or else
  status `BLOCKED`.
- Recovery: read-only; repeatable.
- Compatibility decision: none.

### EP-M1: band model and `DOMAIN` family registration

- Outcome: families carry band tables validated at compile time. `DOMAIN` is
  registered as an experimental, explicit opt-in family with its five bands and
  no rules. Catalogue validation enforces band membership, kind, and uniqueness.
- Requirements: `RFC11-ALLOC`, `AMEND-CATALOGUE`, `CLI-STABLE`, D-5.
- Red: add `bands_tests.rs` (O-BAND-WF run-time cases),
  `domain_family_tests.rs` (O-BAND-TOTAL, O-AMEND), the catalogue-validation
  cases for O-KIND, and the insta snapshot test `domain_allocation_ledger`.
  They fail to compile or fail because the types do not exist.
- Green: add `Band`, `BandPurpose`, `RuleKind`, `ExperimentalPolicy`, the
  `const fn` validator, `band_of`, the `DOMAIN` family entry, and catalogue
  validation for band and kind.
- Acceptance: the new tests pass. The ledger snapshot shows the five bands in
  order with their purposes and kinds. `make check-fmt`, `make lint`, and
  `make test` pass.
- Conformance check: bands match RFC 0011 Table 1 exactly. No lint crate or
  lint group added. No change to existing families' behaviour (3.6.1's tests
  untouched and green).
- Recovery: revert the milestone commit.
- Remaining gaps: selectors do not yet accept `DOMAIN` tokens.
- Compatibility decision: none.

### EP-M2: Verus proof of conservative extension

- Outcome: `verus/whitaker_cli_rule_family_extension.rs` proves
  `O-CONS-EXPAND`, `O-CONS-UNION`, `O-CONS-FOLD`, and `O-REACH`, and runs in the
  `cli` group.
- Requirements: `RFC11-NODEFAULT`, `RFC11-ALLEXP`, `CLI-EXP-IMPLICIT`, D-2.
- Red: write the model and lemma statements with empty bodies. Run
  `./scripts/run-verus.sh cli` and record the open postconditions.
- Green: add proof bodies until the group verifies. Then run the two negative
  controls and record their failures.
- Acceptance: `./scripts/run-verus.sh cli` and `make verus` pass. The
  trusted-assumption grep finds nothing. `make check-fmt`, `make lint`, and
  `make test` pass.
- Conformance check: the model's precedence fold matches `CLI-PREC` and
  `CLI-EXP-IMPLICIT` line by line, with a comment citing each step. If the
  proof shows D-2 is insufficient, stop and escalate.
- Recovery: the sidecar is standalone, so reverting it touches nothing else.
- Remaining gaps: production tie (EP-M3).
- Compatibility decision: none.

### EP-M3: selector integration and behaviour

- Outcome: the selector parser accepts `DOMAIN` and reserved codes, rejects
  gap codes and numeric prefixes, and resolution emits
  `SelectionNotice::NoImplementedRules` for selectors that match nothing. The
  `DOMAIN` family is excluded from `DEFAULT`, from `ALL` without experimental
  mode, and from the curated experimental set.
- Requirements: `CLI-FORMS`, `CLI-CASE`, D-3, D-4, `RFC11-NOGROUP`.
- Red: the O-SEL-PARSE rstest table and prefix proptest, O-CONS-PROD, the
  behaviour feature below, and the configuration-loading end-to-end tests.
- Green: extend the parser and expansion. Thread `SelectionNotice` out of
  resolution. Map `UnallocatedCode` to `OrthoError::Validation` on key
  `lint.select`, `lint.extend-select`, or `lint.ignore`, through
  `cli::error::invalid`.
- End-to-end: write real `whitaker.toml` files to a temporary directory with
  `cap_std`. Load them with `load_with_sources` and validate them. Assert on
  the resulting selection and notices, and on the error text for gap codes.
  This is the outermost boundary available until 3.5.2 wires configuration into
  the `whitaker` binary; binary-level checks are 3.5.2's.
- Acceptance: all new tests pass. The insta snapshot of the gap-code error
  message is reviewed. `make check-fmt`, `make lint`, and `make test` pass.
- Conformance check: no translation of codes; casing errors unchanged;
  existing selector tests from 3.6.1 untouched and green.
- Recovery: revert the milestone commit; EP-M1 and EP-M2 stand alone.
- Remaining gaps: the production fold tie if 3.6.2 has not landed (see
  `O-CONS-PROD`).
- Compatibility decision: none.

Behaviour specification, at `cli/tests/features/domain_rule_family.feature`,
with steps in `cli/tests/domain_rule_family_bdd.rs`:

```gherkin
Feature: DOMAIN rule family selection
  The DOMAIN family is reserved, experimental, and opt-in.

  Scenario: The default selection excludes the DOMAIN family
    Given a catalogue with fixture rule DOMAIN001
    And a whitaker.toml with no lint selection
    When the selection is resolved
    Then no DOMAIN rule is enabled

  Scenario: Selecting DOMAIN without experimental mode enables nothing
    Given a catalogue with fixture rule DOMAIN001
    And a whitaker.toml selecting "DOMAIN"
    When the selection is resolved
    Then no DOMAIN rule is enabled

  Scenario: Selecting DOMAIN with experimental mode enables the family
    Given a catalogue with fixture rules DOMAIN001 and DOMAIN101
    And a whitaker.toml selecting "DOMAIN" with experimental mode on
    When the selection is resolved
    Then DOMAIN001 and DOMAIN101 are enabled

  Scenario: ALL includes DOMAIN only in experimental mode
    Given a catalogue with fixture rule DOMAIN001
    And a whitaker.toml selecting "ALL"
    When the selection is resolved with experimental mode off
    Then no DOMAIN rule is enabled
    When the selection is resolved with experimental mode on
    Then DOMAIN001 is enabled

  Scenario: Implicit experimental mode does not enable DOMAIN
    Given a catalogue with fixture rule DOMAIN001
    And a whitaker.toml with experimental mode on and no lint selection
    When the selection is resolved
    Then no DOMAIN rule is enabled

  Scenario: A reserved code with no rule loads with a notice
    Given the production catalogue
    And a whitaker.toml selecting "DOMAIN001" with experimental mode on
    When the selection is resolved
    Then the configuration is valid
    And a notice reports that "DOMAIN001" matches no implemented rule

  Scenario: A code in a gap is a configuration error
    Given the production catalogue
    And a whitaker.toml selecting "DOMAIN100"
    When the configuration is validated
    Then validation fails for key "lint.select"
    And the message lists the DOMAIN bands
```

The implicit-experimental scenario needs 3.6.2's fold. If 3.6.2 has not landed,
tag it `@requires-3.6.2` and skip it in the step registration, with the same
hand-off note as `O-CONS-PROD`. Assertions belong in `then` steps only; `given`
and `when` steps return `Result` and never assert.

### EP-M4: documentation, ADR, and roadmap

- Outcome: the decisions are recorded, users and developers can find the
  family, and the roadmap item is ticked.
- Steps:
  1. Write
     `docs/adrs/adr-0016-banded-rule-code-allocation-and-opt-in-experimental-families.md`
     (number re-confirmed in EP-M0), covering D-2, D-3, D-4, and D-5 in the
     documentation style guide's ADR format.
  2. Update `docs/whitaker-cli-design.md` §Rule identifiers and selection
     model: add the `DOMAIN` family, banded allocation, gap-code errors,
     reserved-code notices, explicit opt-in families, and the
     conservative-extension contract, citing ADR 0016 and the Verus sidecar.
  3. Update `docs/users-guide.md`: a short "Domain-modelling rules (`DOMAIN`)"
     subsection under rule selection, showing the opt-in TOML and CLI forms,
     the notice for unimplemented codes, and the gap-code error.
  4. Update `docs/developers-guide.md`: how to allocate a code within a
     family band, the band table's re-use policy, how to add a family with
     bands, and how to run the `cli` Verus group.
  5. Add a cross-reference from RFC 0011 §Rule-code allocation to ADR 0016,
     and list ADR 0016 in `docs/contents.md`.
  6. Mark 17.1.1 done in `docs/roadmap.md` and in
     `docs/roadmap-phase-17-domain-amendment.md`.
- Acceptance: `make fmt`, `make markdownlint`, `make nixie`,
  `make spelling`, `make check-fmt`, `make lint`, and `make test` pass.
- Conformance check: every trace chain above resolves to a passing artefact.
  Discoveries are reconciled into the CLI design and ADR.
- Recovery: documentation-only commits; revert individually.
- Compatibility decision: none.

## Concrete steps

Run everything from the worktree root. Capture gate output with `tee`:

```bash
BR=$(git branch --show-current)
make check-fmt 2>&1 | tee /tmp/check-fmt-whitaker-$BR.out
make lint 2>&1 | tee /tmp/lint-whitaker-$BR.out
make test 2>&1 | tee /tmp/test-whitaker-$BR.out
./scripts/run-verus.sh cli 2>&1 | tee /tmp/verus-cli-whitaker-$BR.out
make verus 2>&1 | tee /tmp/verus-whitaker-$BR.out
```

Run the gates sequentially, never in parallel. Rust builds queue for compiler
slots through `build-limits-rustc`; a waiting build is not hung.

Focused test runs while iterating, assuming the 3.6.1 module paths:

```bash
cargo nextest run -p whitaker-cli domain_family 2>&1 | tee /tmp/test-domain-whitaker-$BR.out
cargo insta review -p whitaker-cli
```

Expected Verus success transcript shape:

```plaintext
verification results:: N verified, 0 errors
```

Commit after each milestone, with gates green, using an imperative subject that
names the roadmap item, for example `Register the DOMAIN family bands (17.1.1)`.

## Validation and acceptance

The item is done when all of the following are observable:

- Loading a `whitaker.toml` with `select = ["DEFAULT", "DOMAIN"]` and
  `experimental = true` succeeds and yields a `NoImplementedRules` notice for
  `DOMAIN`.
- Loading `select = ["DOMAIN100"]` fails with a validation error on
  `lint.select` whose message lists the five bands.
- Every pre-existing selector test from 3.6.1 passes unchanged.
- `./scripts/run-verus.sh cli` verifies the new sidecar with no trusted
  assumptions.
- `make check-fmt`, `make lint`, `make test`, and `make verus` pass.

Quality criteria:

- Tests: all new rstest, proptest, insta, and rstest-bdd tests pass.
- Verification: O-BAND-WF and O-BAND-TOTAL discharged on production code;
  O-CONS discharged in Verus; O-CONS-PROD passing, with any 3.6.2 residual
  recorded.
- Lint: `make lint` clean, including Whitaker's own Dylint suite.

## Idempotence and recovery

Every step is repeatable. Tests write only to temporary directories. Each
milestone is one or more commits that can be reverted independently, with EP-M1
as the base for EP-M3. The Verus sidecar is standalone.

## Artefacts and notes

To be filled in during implementation: red and green transcripts, the
negative-control failures for O-BAND-WF, O-CONS, and O-CONS-PROD, and the
reviewed insta snapshots.

## Interfaces and dependencies

Assumed 3.6.1 contract. Confirm in EP-M0 and rename here as needed:

- `RuleCode { family: FamilyId, number: u16 }`, displayed as the family
  followed by the zero-padded three-digit number.
- `FamilyId`, a validated uppercase ASCII identifier.
- `Selector` with variants for `DEFAULT`, `ALL`, family, code, and lint name,
  and a fallible `Selector::parse(&str, &Catalogue)`.
- `Catalogue`, a value (not only a global) listing families and rules, with a
  production constructor and a test-only fixture builder.
- A pure `expand(&Catalogue, &Selector, experimental: bool) -> RuleSet`.

Additions in this item (module paths follow 3.6.1's layout):

```rust
/// A closed interval of rule-code numbers within one family.
pub struct Band { pub start: u16, pub end: u16, pub purpose: BandPurpose, pub kind: RuleKind }

/// Why a band exists; rendered in documentation and error messages.
pub enum BandPurpose { General, BoundaryRefinement, TypeStateSpace, LocalProtocol, FutureLocal, GroupedReport }

/// Whether a rule is a Dylint diagnostic or a `whitaker check` grouped report.
pub enum RuleKind { Diagnostic, Report }

/// How an experimental family participates in implicit `--experimental`.
pub enum ExperimentalPolicy { Curated, ExplicitOptIn }

/// Return `true` when bands are non-empty, in `1..=999`, sorted, and disjoint.
pub const fn bands_are_well_formed(bands: &[Band]) -> bool;

/// Return the band of `family` containing `number`, if any.
pub fn band_of(family: &Family, number: u16) -> Option<&Band>;

/// Report selectors that matched no implemented rule.
pub enum SelectionNotice { NoImplementedRules { selector: String } }
```

`SelectorError` gains `UnallocatedCode { family, number, bands }`.
`CatalogueError` gains `CodeOutsideBands` and `KindMismatch`.

Dependencies: no new external crates. `cli/Cargo.toml` gains dev-dependencies
`googletest`, `pretty_assertions`, `insta`, `rstest-bdd`, and
`rstest-bdd-macros`, all already workspace dependencies.

[^ruff-preview]: Ruff documentation, "Preview", section "Using rules that are
    in preview": a preview rule selected by code, prefix, or `ALL` is not
    enabled until preview mode is on. <https://docs.astral.sh/ruff/preview/>
