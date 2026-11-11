# Add the `whitaker_support_macros` proc-macro crate (roadmap 1.3.1)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`,
`Decision log`, `Outcomes & retrospective`, `Conformance basis`, and
`Verification plan` must be kept up to date as work proceeds.

Status: APPROVED

This document must be maintained in accordance with `AGENTS.md`. The canonical
plan file is
`docs/execplans/1-3-1-add-whitaker-support-macros-proc-macro-crate.md`.

## Deviation D-9: accepted as option (b)

The prototyping milestone EP-M0 ran during planning, ahead of any production
code, and falsified a premise of the governing decision record. **ADR 002's
mandated expansion did not achieve ADR 002's own primary technical
requirement**: the four attributes it specified do not suppress
`unexpected_cfgs`, because that diagnostic is resolved during cfg-expansion,
before the annotated item's own lint levels are in scope. A _sibling_
`#[allow(unexpected_cfgs)]` arrives too late, and an attribute macro cannot
reach an enclosing scope without wrapping the item and changing its semantics.
Two of the remaining three attributes suppressed diagnostics the gated form
never emits, and the third removed the only safety net catching a misspelt lint
name.

The repository owner accepted option D-9(b) on 2026-08-21: amend ADR 002 and
ship a minimal macro whose expansion is the `cfg_attr` gate alone, paired with
one `check-cfg` manifest entry. ADR 002 has been amended accordingly and moved
to `Accepted`. This plan is now approved for implementation.

## Prerequisite discovered by the R-1 spike

A spike implementation, since discarded, answered the one empirical question
this plan left open — and found a second, larger problem.

**Good news.** `#[expect(...)]` does work for Dylint-registered Whitaker lints.
Against a `no_std_fs_operations` library built from current source, item-level
`#[allow]`, item-level `#[expect]`, module-level `#![allow]`, and the
`cfg_attr`-gated `#[expect]` all suppressed correctly, and expectations were
fulfilled with no spurious `unfulfilled_lint_expectations`. Axiom A-4 is
discharged and R-1 is closed.

**Bad news.** The aggregated `whitaker_suite` library ignores lint-level
attributes entirely. A controlled experiment — identical fixture, identical
source revision, identical toolchain, only the loaded library differing —
produced one diagnostic under the individual library (the unannotated control)
and three under the suite, plus a spurious unfulfilled-expectation warning for
every `expect`.

Since `whitaker_suite` is what `whitaker --all`, `make lint-whitaker`, and every
installed consumer load, **no attribute-based suppression currently works in
the configuration this macro targets**. The evidence is in `Artefacts and
notes` §R-1 spike.

This does not block delivery of 1.3.1. The macro's obligations are all
token-level: what it parses, what it rejects, and what tokens it emits. None of
them depend on suite behaviour. It does block the macro from being _useful_,
and therefore blocks ADR 002 migration phase 3. It is recorded in ADR 002
§Known risks and tracked as separate work; see `Decision log` D-13.

## Purpose / big picture

Whitaker enforces project conventions through Dylint lint libraries. Dylint
lints are unknown to `rustc` during ordinary compilation, so a deliberate,
narrowly-scoped exception cannot simply be written as
`#[expect(some_whitaker_lint)]`.

Dylint's answer is conditional compilation: for each lint library it loads it
passes `--cfg=dylint_lib="LIBRARY_NAME"`, so an exception can be written as

```rust,no_run
#[cfg_attr(dylint_lib = "whitaker_suite", expect(no_std_fs_operations, reason = "legacy"))]
fn read_legacy_config() {}
```

That form works, and — given one `check-cfg` entry in the consuming manifest —
is completely warning-free. It is also verbose, easy to misspell, and drifts
between call-sites.

After this change a maintainer writes one attribute:

```rust,no_run
#[whitaker_support_macros::dylint_expect(
    lib = "whitaker_suite",
    lints(no_std_fs_operations),
    reason = "legacy call-site; remove once the cap-std migration lands"
)]
fn read_legacy_config() {}
```

Because `expect` rather than `allow` is used, the suppression announces itself
the moment it becomes stale.

Concretely, once this plan is complete:

1. `crates/whitaker_support_macros` exists as a `proc-macro = true` crate
   exporting one attribute macro, `dylint_expect`, accepting `lib = "..."`,
   `lints(path, ...)`, and an optional `reason = "..."`.
2. The expansion is the `cfg_attr` gate and nothing else, so a misspelt lint
   name still trips `unknown_lints` inside a Dylint run.
3. The workspace manifest carries `cfg(dylint_lib, values(any()))` in its
   `check-cfg` list, and that one line is documented as the prerequisite for
   warning-free use, in this workspace and downstream.
4. Malformed invocations produce precise, span-anchored errors, proven by
   `trybuild` compile-fail fixtures with reviewed `.stderr` snapshots.
5. The crate is publish-ready and wired into the release pipeline, published
   last so it cannot strand the crates users actually consume.
6. ADR 002 is amended and moved to `Accepted`; `docs/repository-layout.md` and
   `docs/whitaker-dylint-suite-design.md` are updated; `docs/roadmap.md` 1.3.1
   is marked done.
7. `make check-fmt`, `make typecheck`, `make lint`, and `make test` all pass.

Note the attribute path. ADR 002 specifies the eventual spelling as
`#[whitaker_support::dylint_expect(...)]`, but the `whitaker_support` facade is
roadmap item 1.3.2. Within 1.3.1 the macro is reached at its own real path,
`#[whitaker_support_macros::dylint_expect(...)]`. That is not a compatibility
shim; 1.3.2 adds the facade re-export without changing anything delivered here.

## Context and orientation

Assume no prior knowledge of this repository.

### The four diagnostics, and which actually fire

ADR 002 §Context names three diagnostics that get in the way. EP-M0 measured
all of them against this workspace's real lint policy. The results are the
foundation of this plan, so they are stated here rather than buried:

| Diagnostic                                | Level here                                                    | Fires on the bare `cfg_attr` gate?                                                          |
| ----------------------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| `unexpected_cfgs`                         | `warn` in `[workspace.lints.rust]`, promoted by `-D warnings` | **Yes** — and only a manifest `check-cfg` entry or an enclosing-scope `allow` suppresses it |
| `unknown_lints`                           | `deny` in `[workspace.lints.rust]`                            | **No** outwith Dylint; **yes** inside a Dylint run if the lint name is wrong                |
| `clippy::allow_attributes`                | `deny` in `[workspace.lints.clippy]`                          | **No** — there are no `allow` attributes to lint                                            |
| `clippy::allow_attributes_without_reason` | `deny` in `[workspace.lints.clippy]`                          | **No** — same reason                                                                        |

_Table 1: Which diagnostics the gated form actually produces._

Two consequences follow, and they drive the whole design. First, only
`unexpected_cfgs` is a real problem, and a macro cannot solve it. Second,
`unknown_lints` firing inside a Dylint run is not an obstacle — it is the only
mechanism that catches a misspelt lint name, and suppressing it would make
every typo a silent no-op.

### Key files a newcomer needs

- `docs/adr-002-dylint-expect-attribute-macro.md` — the governing decision.
  Read §Decision outcome / proposed direction, §Functional requirements, and
  §Known risks. Note that D-9 proposes amending it.
- `docs/roadmap.md` lines 28–47 — the 1.3.x group. 1.3.1 is this plan; 1.3.2
  adds the `whitaker_support` facade; 1.3.3 adds cross-configuration
  compatibility coverage; 1.3.4 completes the narrative documentation.
- `crates/whitaker_test_macros/` — the only other `proc-macro = true` crate in
  the workspace, and the manifest precedent. Thirty-two lines; ignores its
  attribute arguments entirely, so no precedent for parsing or diagnostics.
  See `Surprises & discoveries` S-3: its emitted prelude now trips a Clippy
  lint that did not exist when it was written.
- `Cargo.toml` (root) — `members = ["common", "crates/*", "installer",
  "suite"]`, so a new directory under `crates/` joins the workspace
  automatically. `[workspace.lints.rust]` line 180 holds the `check-cfg` array
  this plan extends.
- `Makefile` — `WHITAKER_PACKAGES` (line 81) lists crates the Whitaker suite
  lints. `typecheck` (223–224), `lint-clippy` (179–181), and `test` (92–139)
  are workspace-wide and need no per-crate edit. `publish-check` (316–348) is
  **not** a quick packaging check — see `Risks` R-7.
- `.config/nextest.toml` line 46 — the `serial-dylint-ui` override matches
  `binary(ui) & test(=ui)`, which the obvious naming for a trybuild harness
  would collide with. See R-8.
- `.github/workflows/ci.yml` line 160 and `.github/workflows/release.yml`
  lines 334–341 — the two places crates are enumerated for packaging and
  publishing. The publish step runs under `set -euxo pipefail` with no
  per-crate guard.
- `rust-toolchain.toml` — pinned to `nightly-2026-05-28`.

### Terms defined

- **Attribute macro**: a procedural macro invoked as `#[name(args)] item`. It
  receives the argument tokens and the item tokens and returns replacement
  tokens.
- **Pre-expansion lint**: a lint running before macro expansion, which
  `cfg_attr` gating cannot help. ADR 002 §Known risks accepts this limitation.
- **`expect` versus `allow`**: `#[allow(L)]` silences `L` forever;
  `#[expect(L)]` silences it but emits `unfulfilled_lint_expectations` if `L`
  never fires, so stale suppressions surface.
- **Silent no-op**: a `dylint_expect` attribute that compiles cleanly and
  suppresses nothing, because the `lib` value or a lint name is wrong. Three
  independent routes to this exist; see R-2.

## Conformance basis

- Governing decision record: `docs/adr-002-dylint-expect-attribute-macro.md`,
  as at commit `02e6c1c`, status `Proposed`. **D-9 proposes amending §Options
  considered, §Decision outcome, and §Known risks before implementation.**
- Roadmap: `docs/roadmap.md` §1.3, item 1.3.1.
- Governing standards: `AGENTS.md`, `docs/documentation-style-guide.md`,
  `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rust-doctest-dry-guide.md`,
  `docs/complexity-antipatterns-and-refactoring-strategies.md`.
- There is no Terms of Reference artefact. ADR 002 is the sole upstream
  requirements source; none has been invented.

ADR 002's requirements are unnumbered in the source, so this plan assigns local
identifiers and quotes the source sentence for each, keeping the mapping
auditable.

| Identifier   | ADR 002 source                                                                                      |
| ------------ | --------------------------------------------------------------------------------------------------- |
| ADR002-FR-1  | "Provide an item attribute usable on functions, impl blocks, modules, and other Rust items."        |
| ADR002-FR-2  | "Support one or multiple lint names per annotation."                                                |
| ADR002-FR-3  | "Support an optional human-readable reason."                                                        |
| ADR002-FR-4  | "Enable `#[expect(...)]` only when Dylint runs the specified lint library."                         |
| ADR002-TR-1  | "Avoid warnings in non-Dylint builds."                                                              |
| ADR002-TR-2  | "Keep the macro's expansion explicit and reviewable."                                               |
| ADR002-TR-3  | "Maintain a clear separation between proc-macro code and lint implementation code."                 |
| ADR002-TR-4  | "Document limitations for 'pre-expansion' lints."                                                   |
| ADR002-MIG-1 | §Migration plan phase 1: "Add `crates/whitaker_support_macros` with the proc-macro implementation." |

_Table 2: Local identifiers assigned to ADR 002's unnumbered requirements._

Trace chains:

```plaintext
ADR002-FR-1 -> EP-M3 -> crates/whitaker_support_macros/tests/applies_to_items.rs
ADR002-FR-2 -> EP-M2 -> expand::tests::preserves_lint_order_and_multiplicity
ADR002-FR-3 -> EP-M2 -> expand::tests::reason_is_propagated_into_expect
ADR002-FR-4 -> EP-M2 -> src/expand/snapshots/*cfg_attr_gate*.snap
ADR002-TR-1 -> EP-M3 -> make lint, contrasted against the EP-M0 transcript
ADR002-TR-2 -> EP-M2 -> insta snapshots of every expansion variant
ADR002-TR-3 -> EP-M1 -> only src/lib.rs names `proc_macro`
ADR002-TR-4 -> 1.3.4 -> deferred by D-10
ADR002-MIG-1 -> EP-M1 -> cargo metadata lists whitaker_support_macros
INV-SHAPE   -> EP-M2 -> keys::tests::exhaustive_key_sequences_to_length_four
```

## Constraints

Hard invariants. Violation requires escalation, not a workaround.

- The macro's argument surface is fixed by ADR 002: `lib = "..."` (string
  literal), `lints(path, ...)` (one or more), optional `reason = "..."`. Do not
  add, rename, or reorder these.
- The expansion emits **only** the `cfg_attr(dylint_lib = "...", expect(...))`
  gate. It must not emit `allow(unknown_lints)`, because that converts every
  misspelt lint name into a silent no-op. _This constraint is contingent on
  D-9(b) being accepted; under D-9(a) it is replaced by ADR 002's four-attribute
  set._
- Only `crates/whitaker_support_macros/src/lib.rs` may name the `proc_macro`
  crate. Every other module operates on `proc_macro2` and `syn` types so it is
  callable from a plain unit test. This is the ADR002-TR-3 boundary.
- No file may exceed 400 lines (`AGENTS.md`).
- `clippy::unwrap_used`, `clippy::expect_used`, `clippy::indexing_slicing`, and
  `clippy::panic_in_result_fn` are denied. The macro returns `syn::Result` and
  converts to `compile_error!` at the single adapter boundary.
- `unsafe_code` is forbidden; `missing_docs` and
  `rustdoc::missing_crate_level_docs` are denied.
- Do not modify any existing lint crate, `common/`, `suite/`, `installer/`, or
  `src/`. This plan adds one crate and edits manifests, workflows, and
  documentation only. The single exception is `crates/whitaker_test_macros`,
  per D-8.
- Comments and documentation use en-GB-oxendict spelling. Markdown prose wraps
  at 80 columns; code blocks at 120.
- Do not mutate the parent process environment in tests.
- Use the shared default Cargo cache; do not create an isolated `CARGO_HOME`.

## Tolerances (exception triggers)

- Scope: more than 20 files or 900 net lines — stop and escalate.
- Interface: any change to the ADR 002 argument surface — stop and escalate.
- Dependencies: the plan adds `syn`, `quote`, and `proc-macro2` only. Any
  further dependency — including `googletest` and `pretty_assertions`, which
  D-11 removes from the earlier draft — requires escalation.
- Iterations: if any gate still fails after 3 targeted fix attempts on one
  milestone, stop and escalate with the `tee`'d log path.
- Ambiguity: if two readings of the amended ADR 002 give materially different
  expansions, present both with trade-offs rather than choosing silently.
- Deviation: if implementation evidence contradicts the amended ADR 002 again,
  set status `BLOCKED` and escalate. Do not amend the implementation around it.

## Risks

- **R-1 — CLOSED, 2026-08-21.** `#[expect(L)]` does work for Dylint-registered
  Whitaker lints. Discharged by the spike recorded in `Artefacts and notes`
  §R-1 spike. Axiom A-4 is established.

- **R-1b — the aggregated suite ignores lint-level attributes.** Superseding
  R-1 as the material risk. `libwhitaker_suite` honours neither `#[allow]` nor
  `#[expect]`, and emits a spurious `unfulfilled_lint_expectations` warning for
  every `expect`, while the individual lint libraries built from the same
  source behave correctly.
  Severity: high. Likelihood: certain — reproduced under a controlled
  experiment.
  Mitigation: none available within 1.3.1, and none needed for delivery, since
  every obligation in this plan is token-level. Tracked as separate work (D-13)
  and recorded in ADR 002 §Known risks. It must be fixed before ADR 002
  migration phase 3 begins adopting the attribute across the estate, or the
  estate will fill with annotations that suppress nothing. Do **not** attempt
  the fix inside this plan: it touches suite wiring that `Constraints` places
  out of scope.

- **R-2 — three independent routes to a silent no-op.** A misspelt lint name, a
  `lib` value naming a library the consumer did not load, or a `lib` written
  with hyphens rather than underscores all produce an attribute that compiles
  cleanly and suppresses nothing.
  Severity: high. Likelihood: high once adoption begins.
  Mitigation: not emitting `allow(unknown_lints)` closes the first route inside
  a Dylint run — that is the main reason D-9(b) is recommended. The other two
  cannot be closed by a macro: they need a lint that inspects call-sites
  against loaded libraries and registered lint names. Three of the six
  reviewers converged independently on this. Book it as roadmap 1.3.5 before
  ADR 002 phase 3 adoption makes the attribute widespread. Meanwhile the wrong
  `lib` case fails closed — the underlying lint fires and CI goes red.

- **R-3 — Whitaker ships two deployment modes, and one `lib` cannot name
  both.** The suite is published both as an aggregated `whitaker_suite` cdylib
  and as per-lint libraries. Which `dylint_lib` cfg is set depends on what the
  consumer installed, so a suppression written for one is inert for the other.
  Severity: medium. Likelihood: high.
  Mitigation: implement the scalar `lib = "..."` form only, but reserve the
  additive escape hatch now by specifying that the `Lib` key may later carry
  either `lib = "x"` or `lib("a", "b")`. Because the key-shape policy quantifies
  over keys and not payloads, that extension costs nothing later, whereas a
  fourth `libs` key would perturb the "exactly one `Lib`" rule. Record the
  reservation in ADR 002 §Outstanding decisions at EP-M5.

- **R-4 — publishing a new, unproven crate first in an all-or-nothing step.**
  `release.yml` runs its publish block under `set -euxo pipefail` with no
  per-crate guard. A failure on the new crate aborts the step, so
  `whitaker-common` and `whitaker-installer` never publish, and crates.io
  publishes cannot be undone.
  Severity: high. Likelihood: medium.
  Mitigation: publish `whitaker_support_macros` **last**, and guard each
  publish so an already-uploaded version is skipped rather than fatal. Both
  names are currently free on crates.io — `whitaker_support_macros` and
  `whitaker_support` return 404 against the registry API, with `whitaker-common`
  returning 200 as a control.

- **R-5 — version drift across ~20 hardcoded manifests.** There is no bump
  tooling in `scripts/`, and `release.yml` verifies the tag against
  `whitaker-installer` only. Missing the new crate in a bump triggers R-4 on a
  release that cannot be retried cleanly.
  Severity: medium. Likelihood: medium.
  Mitigation: use `version.workspace = true` in the new manifest so there is
  one fewer place to drift, and extend the tag-version check to every
  publishable crate.

- **R-6 — the crate is published before it has ever been compiled under
  Dylint.** D-10 defers the Dylint-run configuration to roadmap 1.3.3, so every
  expansion obligation in 1.3.1 is self-referential: the snapshots assert that
  the macro emits what the macro emits.
  Severity: medium. Likelihood: high.
  Mitigation: R-1's probe is the minimum. Prefer completing roadmap 1.3.3
  before the first release tag that would publish this crate; the release
  wiring is inert until then, so this costs nothing to honour.

- **R-7 — `make publish-check` is not a packaging check.** It adds rustup
  components, builds the workspace, runs the entire nextest suite, installs
  cargo-dylint, clones the repository, and builds all ten lint crates in
  release into a cold target directory. Budget 20–40 minutes.
  Severity: low. Likelihood: high.
  Mitigation: use `cargo package -p whitaker_support_macros --allow-dirty`
  (10–20 s) for the EP-M4 loop. The `ci.yml` addition itself is cheap.

- **R-8 — the trybuild harness would silently join a serial test group.**
  `.config/nextest.toml` line 46 matches `binary(ui) & test(=ui)`, which the
  obvious `tests/ui.rs` with `fn ui()` satisfies. It would inherit
  `max-threads = 1` and two exponential retries, so a legitimately failing
  `.stderr` costs three full trybuild runs before reporting.
  Severity: low. Likelihood: high.
  Mitigation: name the harness function something other than `ui`, or add an
  explicit override with `retries = 0`. Decide deliberately, do not inherit.

- **R-9 — snapshot brittleness.** Raw `TokenStream::to_string()` output is
  sensitive to `quote` spacing.
  Severity: low. Likelihood: low.
  Mitigation: snapshot a normalized rendering through one helper, so a
  formatting change is a one-line re-bless.

## Verification plan

The earlier draft carried a Verus sidecar proof, a permutation property test,
seven BDD scenarios, and two new assertion crates. D-11 removes all of them.
The reasoning is recorded here rather than in a footnote, because the removal
is the single largest change in this revision.

### Why the Verus obligation was cut

The argument-key alphabet has three symbols, and a well-shaped sequence has
exactly one `Lib`, exactly one `Lints`, and at most one `Reason`. The longest
accepting sequence therefore has length 3. By the pigeonhole principle any
sequence of length 4 or more contains a repeated symbol, hence a duplicate key,
hence is rejected.

An exhaustive enumeration to length 4 is consequently not merely "complete
within a bound" — it is a **total decision procedure over the infinite
domain**, at 1 + 3 + 9 + 27 + 81 = 121 cases and sub-millisecond cost. Order
independence follows immediately, because counting is order-independent.

The proposed Verus lemma would have modelled the policy as a left fold and the
specification as a multiset predicate, then proved the two agree — two
notations for one decidable property. `AGENTS.md` requires proofs to be
"substantive, rigorous, and well-founded, not merely a restatement of the
assumed property", and that lemma would have been exactly the restatement.

Two further facts confirmed the removal. `make verus` and `make kani` run in no
CI workflow — only `scripts/check-verus-fragment-id-bridge.sh` does — so the
obligation would never have been enforced. And every existing sidecar in
`verus/` has an executable partner (Kani drives the real code while Verus
models it); this one would have been the first with no runtime counterpart,
guarding the most trivial property in the repository.

### Axioms (assumed, not verified)

- **A-1**: `syn` 2.x parses `lib = "..."`, `lints(a, b)`, and `reason = "..."`
  into the token structures its documentation describes. Third-party internals
  are not verified; repository-owned logic built on this interface is verified
  against the real parser.
- **A-2**: `rustc` applies item-level lint attributes in preference to manifest
  `[lints]` levels — **except** for `unexpected_cfgs` arising from an
  attribute on the same item, which EP-M0 showed requires an enclosing scope.
- **A-3**: Dylint passes `--cfg=dylint_lib="LIBRARY_NAME"` for each loaded
  library. Discharged empirically at roadmap 1.3.3, not here.
- **A-4**: `#[expect(L)]` for a Dylint-registered lint behaves as it does for
  built-in lints. **Established empirically, 2026-08-21**, against an
  individual lint library built from current source. Note the scope limit: it
  holds for individual libraries and **not** for the aggregated
  `whitaker_suite` (R-1b), so this axiom supports the macro's design but not
  yet its usefulness in the shipping configuration.

### INV-SHAPE: argument keys are validated exactly and order-independently

- Obligation: validation succeeds if and only if the supplied key sequence
  contains exactly one `Lib`, exactly one `Lints`, and at most one `Reason`.
- Method: exhaustive enumeration over every sequence of length 0–4, plus a
  stated pigeonhole argument in a comment covering all longer sequences.
- Rationale: total, cheap, and directly readable. See above.
- Domain: all of `{Lib, Lints, Reason}*`.
- Artefact: `crates/whitaker_support_macros/src/keys.rs`, test
  `exhaustive_key_sequences_to_length_four`.
- Evidence: `cargo nextest run -p whitaker_support_macros -E
  'test(exhaustive_key_sequences)'`. Fails to compile before `validate_keys`
  exists.
- Non-vacuity: the enumeration covers the empty sequence (rejected,
  `MissingLib`), every singleton (all rejected), both accepting two-element
  orders, all accepting three-element orders, and every duplicate-bearing
  sequence. It asserts the **specific error variant**, so an implementation
  collapsing all failures into one variant is rejected. Negative control:
  swapping `MissingLints` for `MissingLib` in one branch must fail the test.

Note that arity is deliberately **not** part of this obligation. `lints()` with
zero paths supplies the `Lints` key and passes key validation; the empty-list
rejection lives in the parser, where the span needed to report it exists. The
earlier draft called this module `grammar` and claimed it covered the grammar;
it does not, and it is now named `keys` accordingly.

### INV-EXP-1: the annotated item is preserved verbatim

- Obligation: the expansion ends with exactly the input item tokens, nothing
  inserted, removed, or reordered.
- Method: parameterized `rstest` cases across item kinds, plus compile-level
  evidence.
- Rationale: a finite partition over Rust item kinds. A property test over
  arbitrary token trees would test `quote`'s interpolation, a third-party
  internal (A-1).
- Domain: `fn`; `fn` with generics and a where-clause; `impl` block; inherent
  method; `mod`; `struct`; `trait`; and an item that already carries doc
  comments and other attributes.
- Artefact: `crates/whitaker_support_macros/src/expand.rs` tests;
  `crates/whitaker_support_macros/tests/applies_to_items.rs`.
- Evidence: `cargo nextest run -p whitaker_support_macros`.
- Non-vacuity: the "already carries doc comments and attributes" case fails if
  the implementation re-parses and re-emits the item instead of passing tokens
  through. Negative control: make the expansion drop existing attributes and
  confirm that case fails.

### INV-EXP-2: the expansion is exactly the gate

- Obligation: the expansion emits the `cfg_attr` gate and nothing else, with
  the lint paths in source order and the reason present only when supplied.
- Method: `insta` snapshots over a normalized rendering, one per variant.
- Rationale: ADR002-TR-2 requires the expansion be "explicit and reviewable",
  and this is the multivariant output-consistency case snapshots exist for.
- Domain: single lint; multiple lints; with reason; without reason; a library
  name containing underscores.
- Artefact: `crates/whitaker_support_macros/src/expand.rs`, snapshots in
  `crates/whitaker_support_macros/src/snapshots/` — `insta` resolves snapshot
  directories relative to the test file, so this follows the test module's
  location, matching `crates/whitaker_clones_core/src/ast/snapshots`.
- Evidence: `cargo nextest run -p whitaker_support_macros` with
  `INSTA_UPDATE=no`; new snapshots are unreviewed until blessed.
- Non-vacuity: all five variants must differ from one another, so an
  implementation ignoring `reason` or flattening the lint list produces
  identical snapshots for distinct inputs. Negative control: add a stray
  `allow` to the expansion and confirm all five fail.

### INV-EXP-3: lint order and multiplicity are preserved

- Obligation: the paths inside `expect(...)` are exactly those given to
  `lints(...)`, same order, same multiplicity.
- Method: `proptest` over generated path lists.
- Rationale: an invariant over arbitrary-length lists, so a property test is
  proportionate. Silently deduplicating or sorting would make the expansion
  diverge from the call-site a reviewer reads.
- Domain: generated lists of length 1–8 from a pool that deliberately contains
  repeats.
- Artefact: `crates/whitaker_support_macros/src/expand.rs`; regression seeds
  under `crates/whitaker_support_macros/proptest-regressions/`.
- Evidence: `cargo nextest run -p whitaker_support_macros`. Honour
  `PROPTEST_CASES` so the case count is tunable without editing code.
- Non-vacuity: record classification showing at least 20% of generated cases
  contain a repeat; a lower rate is a generator defect, not a pass. Negative
  control: insert `.dedup()` and confirm the property shrinks to a two-element
  repeated list.

### INV-DIAG-1: malformed invocations produce specific diagnostics

- Obligation: each malformed-argument class produces a distinct, span-anchored
  error naming the offending argument.
- Method: `trybuild` compile-fail fixtures with reviewed `.stderr` snapshots.
- Rationale: diagnostic text and span placement are only observable through a
  real compilation. This is also the **only** compatibility net this API will
  ever have: `cargo-semver-checks` inspects rustdoc and is blind to a proc
  macro's argument grammar.
- Domain: missing `lib`; missing `lints`; empty `lints()`; duplicate `lib`;
  duplicate `lints`; duplicate `reason`; non-string-literal `lib`;
  non-string-literal `reason`; unknown argument key; `lints` given as a string
  rather than a list; a non-path inside `lints(...)`; a path with generics or a
  leading `::`; `lib = ""`; `lib` containing a hyphen; trailing commas in both
  `lints(a,)` and the top-level list.
- Artefact: `crates/whitaker_support_macros/tests/ui.rs` (harness function
  **not** named `ui`, per R-8) with fixtures under `tests/ui/`.
- Evidence: `cargo nextest run -p whitaker_support_macros -E
  'binary(ui)'`. Each fixture fails with no `.stderr` present, then passes once
  a reviewed `.stderr` is blessed.
- Non-vacuity: the earlier draft's control — "all `.stderr` files must differ"
  — was itself vacuous, because trybuild embeds the fixture path and line
  number in every file, so they always differ. Compare **normalized message
  text** with paths and line numbers stripped. Note that an empty argument list
  and a missing `lib` genuinely collapse onto the same error today; either add
  an `EmptyArguments` variant or record the collapse as deliberate, rather than
  letting a broken control paper over it.

### INV-WARN-1: warning-free with the check-cfg entry present

- Obligation: applying the attribute to any supported item kind produces no
  diagnostic under `cargo check` or `cargo clippy` with warnings denied and
  this workspace's full lint policy in force.
- Method: compile-level evidence from the repository's own gates. The crate's
  integration tests and rustdoc examples use the macro, are compiled under
  `RUSTFLAGS="-D warnings"` by `make test` and under `cargo clippy -- -D
  warnings` by `make lint-clippy`, and inherit `[lints] workspace = true`. No
  bespoke harness is needed.
- Rationale: this is ADR002-TR-1, and the honest check is to compile real
  usages under the exact policy the workspace enforces.
- Domain: the non-Dylint configuration only. The Clippy-run and Dylint-run
  matrix is roadmap 1.3.3's scope, per D-10.
- Artefact: `crates/whitaker_support_macros/tests/applies_to_items.rs`, the
  crate's rustdoc examples, and the gates.
- Evidence: `make lint` exits 0 with no warning mentioning the new crate.
- Non-vacuity: EP-M0 established that the same code **does** warn without the
  `check-cfg` entry. That transcript is the negative control, and it is
  recorded in `Artefacts and notes`. Without it a clean `make lint` proves
  nothing.

### Stacking

Two `dylint_expect` attributes on one item is the only route to covering both
deployment modes until R-3's extension lands, so it must be a tested,
documented case. Under D-9(b) the expansion is a single `cfg_attr` with no
`allow` attributes, so stacking cannot produce duplicate attributes and
`clippy::duplicated_attributes` has nothing to fire on — but assert it rather
than assume it.

## Plan of work

### Stage A — complete EP-M0

Probes 2, 3, and 4 were run during planning; their transcripts are in
`Artefacts and notes`. Probe 1 (R-1) remains open and requires a real Dylint
session. Commit the probes as `scripts/probe-dylint-expect-viability.sh` with
asserted exit codes, so the gate is a re-runnable artefact rather than
self-attested prose, and so that INV-WARN-1's negative control cannot evaporate.

Go/no-go: if probe 1 shows `expect` does not work for Dylint lints, stop and
escalate — ADR 002 needs a second amendment.

### Stage B — red tests and the specification

1. `crates/whitaker_support_macros/Cargo.toml` with `[lib] proc-macro = true`
   and full publish metadata.
2. `src/lib.rs` containing only crate documentation, module declarations, and a
   `dylint_expect` that returns `compile_error!("not yet implemented")`.
3. All unit, exhaustive, property, and snapshot tests, written against the
   intended API. They will not compile — that is the red state.
4. All compile-fail fixtures with no `.stderr` files.

Validation: `cargo nextest run -p whitaker_support_macros` fails to compile
with errors naming the missing items. Record the transcript.

### Stage C — implementation

1. `src/keys.rs` — `ArgKey`, `ArgShapeError`, and `validate_keys`. Turn the
   exhaustive test green.
2. `src/args.rs` — the `syn::parse::Parse` implementation mapping tokens to
   keyed payloads, calling `validate_keys`, validating payloads (non-empty
   lint list, path shape, non-empty `lib`, underscore-only `lib`), then
   assembling `DylintExpect`.
3. `src/expand.rs` — the renderer. Turn INV-EXP-1 through INV-EXP-3 green and
   bless the snapshots after reading each one.
4. `src/lib.rs` — wire the adapter and convert `syn::Error` via
   `to_compile_error()`. Bless the `.stderr` fixtures after reviewing each for
   span placement and wording, comparing normalized message text.

Validation: `cargo nextest run -p whitaker_support_macros` passes.

### Stage D — wiring and documentation

1. Root `Cargo.toml`: add `cfg(dylint_lib, values(any()))` to the
   `[workspace.lints.rust]` `check-cfg` array; add `syn`, `quote`, and
   `proc-macro2` to `[workspace.dependencies]`; add the
   `whitaker_support_macros` entry.
2. `crates/whitaker_test_macros/Cargo.toml`: migrate to the shared pins (D-8).
3. `Makefile`: append `-p whitaker_support_macros` to `WHITAKER_PACKAGES`.
4. `.github/workflows/ci.yml` line 160: add the crate to `PUBLISH_PACKAGES`.
5. `.github/workflows/release.yml`: add `cargo publish -p
   whitaker_support_macros` as the **last** publish step, with an
   already-published guard on every step in the block (R-4).
6. Documentation: amend and accept ADR 002; cross-reference from
   `docs/whitaker-dylint-suite-design.md`; add the crate to
   `docs/repository-layout.md`; mark `docs/roadmap.md` 1.3.1 done. The users'
   and developers' guide narratives are deferred to 1.3.4 per D-10.
7. Run the full gate set.

Validation: `make check-fmt`, `make typecheck`, `make lint`, `make test`,
`make markdownlint`, and `make nixie` all pass.

## Milestones and plateaus

### EP-M0 — prototype findings recorded (prototyping milestone)

- Outcome: probes 2–4 answered with transcripts; probe 1 (R-1) explicitly open;
  probes committed as a script with asserted exit codes.
- Requirements and gaps: de-risks ADR002-FR-4 and ADR002-TR-1; targets A-4.
- Acceptance evidence: EV-M0 — `scripts/probe-dylint-expect-viability.sh` exits
  0, and its recorded output matches `Artefacts and notes`.
- Conformance check: **failed at planning time.** The evidence contradicts
  ADR 002 §Decision outcome; D-9 records the deviation and the plan is BLOCKED
  pending acceptance.
- Recovery: the probe script is additive and independently revertible.
- Remaining gaps: probe 1; everything downstream.
- Compatibility decision: none required.

### EP-M1 — crate skeleton exists and the workspace still builds

- Outcome: the crate is a workspace member with correct metadata and lint
  inheritance; `make typecheck` passes; the macro is a stub that always errors.
- Requirements and gaps: ADR002-MIG-1, ADR002-TR-3.
- Acceptance evidence: EV-M1 — `cargo metadata --format-version 1 --no-deps`
  lists the crate, and `make typecheck` exits 0.
- Conformance check: only `src/lib.rs` names `proc_macro`; `version.workspace =
  true`; `rust-version` declared; no dependency beyond the three approved.
- Recovery: delete the directory; the `crates/*` glob makes removal complete.
- Remaining gaps: all behaviour.
- Compatibility decision: none. New crate, no consumers.

### EP-M2 — the macro is correct

- Outcome: parsing, validation, and expansion are correct; every test passes;
  fixtures and snapshots are reviewed and blessed; R-1's probe has answered.
- Requirements and gaps: ADR002-FR-1 through FR-4, ADR002-TR-2; INV-SHAPE,
  INV-EXP-1, INV-EXP-2, INV-EXP-3, INV-DIAG-1.
- Acceptance evidence: EV-M2 — `cargo nextest run -p whitaker_support_macros`
  reports all tests passed with the count recorded. Every negative control has
  been run and reverted, with its failing output recorded.
- Conformance check: the expansion matches the amended ADR 002 exactly; no file
  exceeds 400 lines; no `unwrap`/`expect` in non-test code.
- Recovery: snapshots and `.stderr` files regenerate with `INSTA_UPDATE=always`
  and `TRYBUILD=overwrite`, but must be re-read before committing.
- Remaining gaps: wiring, release, documentation.
- Compatibility decision: none.

### EP-M3 — warning-free under the full workspace lint policy

- Outcome: `make lint` and `make test` pass with the crate included and the
  `check-cfg` entry present.
- Requirements and gaps: ADR002-TR-1; INV-WARN-1.
- Acceptance evidence: EV-M3 — `make lint` exits 0 with no warning referencing
  the crate, contrasted against the EP-M0 transcript.
- Conformance check: if adding the crate to `WHITAKER_PACKAGES` breaks the
  Dylint check build, as it may for a proc-macro crate, record the failure,
  revert that one line, and note the deviation — do not weaken any lint.
- Recovery: revert the `Makefile` line; the rest stands.
- Remaining gaps: release, documentation.
- Compatibility decision: none.

### EP-M4 — publish-ready and wired into release

- Outcome: `cargo package -p whitaker_support_macros --allow-dirty` succeeds;
  `ci.yml` and `release.yml` include the crate, published last and guarded.
- Requirements and gaps: resolves ADR 002 §Outstanding decisions item 3.
- Acceptance evidence: EV-M4 — the packaging step completes and the `.crate`
  lists `src/`, `Cargo.toml`, and the licence, with no nested manifest.
- Conformance check: the new crate publishes **after** every existing one; each
  publish skips an already-uploaded version rather than aborting; the tag
  version check covers every publishable crate.
- Recovery: revert the two workflow edits. **This is the one milestone whose
  rollback expires** — after the first release tag, publication is irreversible
  and the crates.io name is permanent.
- Remaining gaps: documentation.
- Compatibility decision: none. First publication.

### EP-M5 — documentation and roadmap

- Outcome: ADR 002 amended and `Accepted` with a dated summary; suite design
  cross-referenced; repository layout updated; roadmap 1.3.1 marked `[x]`.
- Requirements and gaps: ADR002-TR-4 is explicitly **deferred to 1.3.4**.
- Acceptance evidence: EV-M5 — `make markdownlint` and `make nixie` pass, and
  `rg -n 'dylint_expect' docs/` lists ADR 002, the suite design, the repository
  layout, and this plan.
- Conformance check: ADR 002 §Options considered must record that Option D's
  original rejection rationale was factually wrong; §Decision outcome must
  carry the amended expansion; §Known risks must carry R-2 and R-3.
- Recovery: documentation edits are independently revertible.
- Remaining gaps: roadmap 1.3.2, 1.3.3, 1.3.4, and the proposed 1.3.5 lint.
- Compatibility decision: none.

## Interfaces and dependencies

### Crate layout

```plaintext
crates/whitaker_support_macros/
├── Cargo.toml
├── src/
│   ├── lib.rs         # adapter: the ONLY file naming `proc_macro`
│   ├── model.rs       # domain value types
│   ├── keys.rs        # pure key-shape policy + exhaustive tests
│   ├── args.rs        # syn adapter: tokens -> validated DylintExpect
│   ├── expand.rs      # renderer + unit, snapshot, property tests
│   └── snapshots/     # insta snapshots, resolved relative to src/
└── tests/
    ├── applies_to_items.rs
    ├── ui.rs          # trybuild harness; fn NOT named `ui` (R-8)
    └── ui/            # compile-fail fixtures plus .stderr
```

Flat modules, not directories. The earlier draft used `args/mod.rs` and
`args/grammar/mod.rs` on the belief that
`clippy::self_named_module_files` requires the `mod.rs` style. It does not — it
forbids `args/args.rs`. A flat `src/args.rs` with no sibling directory is fully
compliant, and every file here sits well under 400 lines.

On the boundary: the infrastructure in a procedural macro is
`proc_macro::TokenStream`, which exists only inside a compiler invocation and
cannot be constructed in a unit test. Everything else — `syn`, `quote`,
`proc_macro2` — is pure compile-time data with no ambient effects, so it
belongs to the domain's vocabulary rather than being something to abstract
away. `src/lib.rs` is therefore the sole adapter and every other module is
directly unit-testable. That is the whole of the architectural claim; it is the
standard `proc_macro2` hygiene idiom, and calling it "hexagonal" would invite a
future contributor to add a trait to complete a pattern that has no second
implementation to abstract over.

### Signatures that must exist at the end of EP-M2

In `crates/whitaker_support_macros/src/keys.rs`:

```rust
/// Identifies which keyword an argument in a `dylint_expect` list supplied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArgKey {
    Lib,
    Lints,
    Reason,
}

/// Describes why a sequence of argument keys is malformed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArgShapeError {
    Duplicate(ArgKey),
    MissingLib,
    MissingLints,
}

/// Accepts a key sequence with exactly one `lib`, exactly one `lints`, and at
/// most one `reason`, in any order. The `usize` locates the offending key so
/// the caller can anchor a diagnostic span on it.
pub(crate) fn validate_keys(keys: &[ArgKey]) -> Result<(), (usize, ArgShapeError)>;
```

The index is load-bearing. `Duplicate(ArgKey)` alone says _which key_ but never
_which occurrence_, forcing the parser to re-scan to find the second `lib` to
point at. Returning the position keeps the policy pure and the diagnostics
precise, and hardens before ten `.stderr` files are blessed.

In `crates/whitaker_support_macros/src/model.rs`:

```rust
/// Names the Dylint library whose `dylint_lib` cfg gates the expectation.
///
/// Construction rejects an empty name and any name containing a hyphen, since
/// Dylint injects a Rust identifier and a hyphenated package name silently
/// produces a suppression that never applies.
pub(crate) struct LibraryName(syn::LitStr);

/// Carries the human-readable justification for a suppression.
pub(crate) struct Reason(syn::LitStr);

/// Holds a validated `dylint_expect` invocation.
pub(crate) struct DylintExpect {
    lib: LibraryName,
    lints: Vec<syn::Path>,
    reason: Option<Reason>,
}
```

Both newtypes hold `syn::LitStr` rather than `String`, so the span survives for
diagnostics and `LibraryName` has a real invariant to enforce — which is the
one route to R-2's silent no-op that the macro _can_ close by itself.

In `crates/whitaker_support_macros/src/expand.rs`:

```rust
/// Renders the cfg-gated expectation followed by the untouched item.
pub(crate) fn expand(spec: &DylintExpect, item: &proc_macro2::TokenStream) -> proc_macro2::TokenStream;
```

In `crates/whitaker_support_macros/src/lib.rs`:

```rust
#[proc_macro_attribute]
pub fn dylint_expect(attr: TokenStream, item: TokenStream) -> TokenStream;
```

### Required expansion

For `lib = "whitaker_suite"`, `lints(no_std_fs_operations, module_max_lines)`,
`reason = "legacy call-site"`:

```rust,no_run
#[cfg_attr(
    dylint_lib = "whitaker_suite",
    expect(no_std_fs_operations, module_max_lines, reason = "legacy call-site")
)]
fn read_legacy_config() {}
```

When `reason` is omitted, the `reason = "..."` key is omitted from
`expect(...)`. Nothing else is emitted. _This is the D-9(b) expansion; under
D-9(a) it would instead be ADR 002's four-attribute set._

Note the library name. The earlier draft used `whitaker_lints` throughout,
copied from ADR 002. No such library exists: `rg whitaker_lints` finds nothing
outwith ADR 002 and this plan. The real names are `whitaker_suite`
(`suite/Cargo.toml`, `installer/src/resolution.rs`) and the individual lint
crates listed in `Makefile` `LINT_CRATES`. Shipping the ADR's string as the
canonical example would have made every copied call-site a guaranteed silent
no-op, and it must be corrected in ADR 002 too.

Lint paths are accepted as `syn::Path` but validated to Dylint's actual shape:
a single-segment identifier, no leading `::`, no generics. Dylint registers
plain names, so a tool-qualified path such as `clippy::needless_return` gated
on `dylint_lib` would be absent during every Clippy run — the only run where it
could fire. Accepting it is a footgun, not future-proofing.

### Dependencies

`crates/whitaker_support_macros/Cargo.toml`:

```toml
[package]
name = "whitaker_support_macros"
version.workspace = true
edition = "2024"
rust-version = "1.81"
description = "Attribute macro for conditional Dylint expect suppressions"
license.workspace = true
repository.workspace = true
homepage.workspace = true
documentation.workspace = true
keywords = ["dylint", "lint", "macro", "expect"]
categories = ["development-tools"]

[lib]
proc-macro = true

[dependencies]
proc-macro2 = { workspace = true }
quote = { workspace = true }
syn = { workspace = true }

[dev-dependencies]
insta = { workspace = true }
proptest = { workspace = true }
rstest = { workspace = true }
trybuild = { workspace = true }

[lints]
workspace = true
```

`rust-version = "1.81"` is a contract, not decoration: the expansion relies on
`#[expect]` and `reason =` in lint attributes, both stabilized in 1.81.

New `[workspace.dependencies]` entries in the root `Cargo.toml`:

```toml
proc-macro2 = "1.0.106"
quote = "1.0.46"
syn = { version = "2.0.119", default-features = false, features = ["derive", "parsing", "printing", "proc-macro"] }
whitaker_support_macros = { path = "crates/whitaker_support_macros", version = "0.2.7" }
```

`syn` is pinned **without** the `full` feature. The item is passed through as an
opaque `proc_macro2::TokenStream` (INV-EXP-1), so only `Path`, `LitStr`,
`Punctuated`, and `parenthesized!` are parsed. `full` is the expensive feature,
and a `[workspace.dependencies]` feature set is baked into the published
manifest — so once roadmap 1.3.2 puts this crate on downstream build graphs,
`full` would cost every consumer 5–10 s of cold compile for nothing.
`crates/whitaker_test_macros` genuinely needs `full` and is `publish = false`,
so it adds that feature at its own use site.

Verify the exact current versions with `cargo search` before pinning; the
values above come from `Cargo.lock` at planning time.

Also add to `[workspace.lints.rust]`:

```toml
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(kani)', 'cfg(dylint_lib, values(any()))'] }
```

This one line is what actually makes the gated form warning-free. Everything
the macro does is ergonomics on top of it, and the plan should not pretend
otherwise.

## Concrete steps

Run everything from the repository root,
`/home/leynos/.lody/repos/github---leynos---whitaker/worktrees/c0b1e9dd-2aff-4206-8bb8-19335d3fa354`.

Gate output is long and the terminal truncates the middle, so pipe through
`tee`:

```bash
set -o pipefail
make check-fmt 2>&1 | tee "/tmp/check-fmt-whitaker-$(git branch --show-current | tr '/' '-').out"
```

### Focused loops

```bash
cargo nextest run -p whitaker_support_macros 2>&1 | tee /tmp/nextest-support-macros.out
cargo nextest run -p whitaker_support_macros -E 'test(exhaustive_key_sequences)'
TRYBUILD=overwrite cargo nextest run -p whitaker_support_macros -E 'binary(ui)'
INSTA_UPDATE=always cargo nextest run -p whitaker_support_macros
cargo insta review
cargo package -p whitaker_support_macros --allow-dirty
```

`TRYBUILD=overwrite` and `INSTA_UPDATE=always` regenerate expected output.
Never commit output blessed this way without reading every regenerated file —
blessing blind converts a test into a tautology.

Use `cargo package` for the EP-M4 loop, **not** `make publish-check` (R-7).

### Full gates

```bash
make check-fmt 2>&1 | tee /tmp/check-fmt-support-macros.out
make typecheck 2>&1 | tee /tmp/typecheck-support-macros.out
make lint      2>&1 | tee /tmp/lint-support-macros.out
make test      2>&1 | tee /tmp/test-support-macros.out
make markdownlint 2>&1 | tee /tmp/markdownlint-support-macros.out
make nixie        2>&1 | tee /tmp/nixie-support-macros.out
```

Delegate full gate runs to the `scrutineer` sub-agent rather than running them
in the planning context; it runs them sequentially, captures each log, and
returns a bounded report. Do not run gates in parallel — this environment uses
build caching, and sequential execution is what benefits from it.

## Validation and acceptance

A reader can confirm this work as follows.

Apply the attribute to a function in the crate's own test tree and run
`make test`, then `make lint`. Expect a clean pass with no warnings. That is
INV-WARN-1. Then remove `cfg(dylint_lib, values(any()))` from the workspace
`check-cfg` array and re-run: expect `unexpected_cfgs` at the call-site. That
contrast is the point of the whole change, and it is the negative control that
makes the clean run meaningful.

Remove the `lints(...)` argument from a fixture and expect a compiler error
reading ``dylint_expect` requires a `lints(...)` argument with at least one lint
path`` anchored at the attribute's span.

### Red-Green-Refactor evidence to record

- Red: `cargo nextest run -p whitaker_support_macros` at the end of Stage B
  fails to compile, with errors naming `validate_keys`, `expand`, and
  `DylintExpect` as unresolved. That is the intended failure reason.
- Green: the same command at the end of Stage C reports all tests passed.
  Record the exact count.
- Refactor: after any extraction, re-run the focused command and then
  `make lint`; both must pass unchanged.

### Quality criteria

- Tests: `make test` passes; `cargo nextest run -p whitaker_support_macros`
  passes.
- Verification: INV-SHAPE, INV-EXP-1 through INV-EXP-3, INV-DIAG-1, and
  INV-WARN-1 discharged by their named artefacts, each with its negative
  control run and recorded. R-1's probe answered.
- Lint and typecheck: `make check-fmt`, `make typecheck`, `make lint` exit 0.
- Documentation: `make markdownlint`, `make nixie` exit 0.
- Packaging: `cargo package -p whitaker_support_macros` exits 0.
- Performance: no threshold, but `syn` must not carry `full`.
- Security: none beyond the workspace's `unsafe_code = "forbid"`.

## Idempotence and recovery

Every step is re-runnable. The crate directory can be deleted and recreated
without touching any other member, because `crates/*` globbing means there is
no `members` list to keep in step.

Snapshot and `trybuild` fixtures are regenerable, but regeneration is not
recovery — a regenerated expectation must be read before it is committed.

Nothing in this plan publishes anything: `cargo publish` runs only from
`release.yml` on a release tag, so the release wiring is inert until a tag is
pushed. **After that first tag it is irreversible**, which is why R-4's
ordering and guards are not optional.

## Artefacts and notes

### EP-M0 transcripts

All commands below were run on the pinned toolchain during planning and are
reproducible in under a minute. Probe 1 (R-1) is **not** among them and remains
open.

**Probe 3 — the unmitigated baseline, and INV-WARN-1's negative control.**
A bare gated attribute, no `allow` attributes, warnings denied,
`check-cfg = 'cfg(kani)'` only:

```plaintext
error: unexpected `cfg` condition name: `dylint_lib`
 --> probe.rs:1:12
  |
1 | #[cfg_attr(dylint_lib = "whitaker_suite", expect(no_std_fs_operations, reason = "legacy"))]
  |            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  = note: `-D unexpected-cfgs` implied by `-D warnings`
```

One diagnostic. No `unknown_lints`: a false `cfg_attr` predicate is stripped
before lint-attribute processing, so `rustc` never sees the unknown lint name.
No `clippy::allow_attributes`: there are no `allow` attributes to lint.

**Probe 3b — the same file with `dylint_lib` added to `check-cfg`.** Exit 0,
no output. This is the entire fix.

**Probe 3c — the cfg active with an unregistered lint name.**

```plaintext
error: unknown lint: `no_std_fs_operations`
 --> probe.rs:1:50
  = note: `-D unknown-lints` implied by `-D warnings`
```

This is the misspelt-lint safety net. ADR 002's mandated
`#[allow(unknown_lints)]` would remove it, converting every typo into a silent
no-op. That is the strongest single argument for D-9(b).

**Probe 3d — where `allow(unexpected_cfgs)` actually works.** Three placements
of the same suppression against the same gated attribute:

```plaintext
(a) allow as a SIBLING attribute on the same item   -> WARNS
(b) allow on an ENCLOSING module                    -> silent
(c) allow as an INNER attribute of the item's body  -> silent
```

Only the ADR 002 shape fails. `unexpected_cfgs` is resolved during
cfg-expansion, before the item's own lint levels are in scope.

**Probe 3e — the decisive one.** A real `proc-macro = true` crate emitting
ADR 002's exact four-attribute expansion, consumed by a second crate carrying
this workspace's lint policy:

```plaintext
warning: unexpected `cfg` condition name: `dylint_lib`
 --> src/lib.rs:1:1
```

The macro, built and run exactly as ADR 002 specifies, still warns — at the
call-site. This is the finding that blocks the plan.

**Probe 2 — Clippy self-suppression.** `#[allow(clippy::allow_attributes,
reason = "...")]` does suppress `clippy::allow_attributes` for its own
attribute list, so ADR 002's shape would have worked had it been needed. Under
D-9(b) it is not needed, because no `allow` attributes are emitted.

**Probe 4 — Kani.** Moot under D-11, which removes the bounded-model-checking
question entirely.

### R-1 spike

A throwaway crate was compiled under a real Dylint session against libraries
built from this worktree's source, then discarded. The fixture annotated
identical `std::fs::read_to_string` call-sites four ways and compared which
were suppressed.

Against `libno_std_fs_operations` — an individual lint library:

```plaintext
error: std::fs operation ... bypasses the capability-based filesystem policy.
 --> src/lib.rs:2:46          <- a_none, no attribute (the control)
error: could not compile `spike_expect` (lib) due to 1 previous error
```

One diagnostic, from the unannotated control. Item-level `#[allow]`,
item-level `#[expect]`, module-level `#![allow]`, and the `cfg_attr`-gated
`#[expect]` were all suppressed, and no `unfulfilled_lint_expectations` warning
was emitted for the fulfilled expectations. **R-1 is answered: `expect` works.**

The controlled experiment, same fixture and source, only the library differing:

```plaintext
########## LIBRARY: no_std_fs_operations ##########
error: LINT FIRED
 --> src/lib.rs:2:46          <- control only
########## LIBRARY: whitaker_suite ##########
error: LINT FIRED
 --> src/lib.rs:2:46          <- control
error: LINT FIRED
 --> src/lib.rs:5:52          <- #[allow(no_std_fs_operations)]  IGNORED
error: LINT FIRED
 --> src/lib.rs:8:53          <- #[expect(no_std_fs_operations)] IGNORED
warning: this lint expectation is unfulfilled
 --> src/lib.rs:7:10          <- spurious
```

Both libraries were built from the same commit with
`cargo build --release --features dylint-driver`. The aggregated suite ignores
lint levels; the individual library honours them. This is R-1b.

Ruled out as causes: staleness of the installed release build (the suite was
rebuilt from source and behaved identically), lint-identity mismatch
(`suite/src/lints.rs` registers the same `&'static Lint` statics the
constituent passes emit with), and `cfg_attr` interaction (plain `#[allow]`
with no gating fails too). The remaining suspect is the
`declare_combined_late_lint_pass!` aggregation in `suite/src/driver.rs`, but
root-causing it is out of scope here and belongs to D-13.

**Crates.io name availability**, checked against the registry API:

```plaintext
whitaker_support_macros -> 404   (free)
whitaker_support        -> 404   (free)
whitaker-common         -> 200   (control: exists)
```

### Compile-fail fixture inventory

Fixtures under `crates/whitaker_support_macros/tests/ui/`, each paired with a
reviewed `.stderr`: `missing_lib.rs`, `missing_lints.rs`, `empty_lints.rs`,
`duplicate_lib.rs`, `duplicate_lints.rs`, `duplicate_reason.rs`,
`non_string_lib.rs`, `non_string_reason.rs`, `unknown_argument.rs`,
`empty_argument_list.rs`, `lints_as_string.rs`, `non_path_in_lints.rs`,
`generic_lint_path.rs`, `leading_colons_lint_path.rs`, `empty_lib.rs`,
`hyphenated_lib.rs`, `trailing_comma_in_lints.rs`, and
`trailing_comma_top_level.rs`.

Compare **normalized** message text across them, with paths and line numbers
stripped; trybuild embeds the fixture path in every `.stderr`, so raw file
comparison always shows a difference and proves nothing.

## Progress

- [x] (2026-08-21) EP-M0 probes 2, 3, and 4 run; transcripts recorded above.
- [x] (2026-08-21) D-9 deviation accepted as option (b) by the repository
      owner. ADR 002 amended and moved to `Accepted`.
- [x] (2026-08-21) R-1 discharged by spike: `#[expect(...)]` works for
      Dylint-registered lints. Spike discarded as planned. Discovered R-1b —
      the aggregated suite ignores lint-level attributes — recorded in ADR 002
      §Known risks and tracked as D-13.
- [ ] EP-M0 remaining: commit the probes as a script with asserted exit codes,
      so INV-WARN-1's negative control is a re-runnable artefact.
- [ ] EP-M1: crate skeleton, manifest, workspace dependency and `check-cfg`
      entries.
- [ ] EP-M2: keys, parser, expansion, all tests green, snapshots and `.stderr`
      fixtures reviewed and blessed.
- [ ] EP-M3: warning-free under the full workspace lint policy.
- [ ] EP-M4: packaging verified; `ci.yml` and `release.yml` wired, new crate
      published last and guarded.
- [ ] EP-M5: ADR 002 amended and accepted; suite design and repository layout
      updated; roadmap 1.3.1 marked done.

## Surprises & discoveries

- **S-1 — ADR 002's expansion does not suppress `unexpected_cfgs`.**
  Evidence: probes 3d and 3e above. Impact: blocks the plan; drives D-9. The
  only working mitigations are a manifest `check-cfg` entry or an
  enclosing-scope `allow`, neither of which an attribute macro can provide for
  an arbitrary item.

- **S-2 — the diagnostics ADR 002 sets out to suppress mostly do not fire.**
  Evidence: probe 3. Impact: `allow(unknown_lints)` and
  `allow(clippy::allow_attributes)` address diagnostics the gated form never
  emits outwith Dylint, and the former destroys the misspelt-lint safety net
  inside Dylint. ADR 002 §Options considered rejects Option D partly on this
  basis, so that rejection rationale must be corrected.

- **S-3 — `whitaker_test_macros` now emits a pattern that trips a Clippy
  lint.** Its expansion uses `#[cfg_attr(clippy, expect(clippy::allow_attributes,
  ...))]`, which the current toolchain rejects with
  `clippy::unnecessary_clippy_cfg` ("no need to put clippy lints behind a
  `clippy` cfg"). Evidence: probe 2's scratch crate. Impact: out of scope here,
  but worth raising as separate follow-up work before it surfaces in a gate.

- **S-4 — the library name in ADR 002 does not exist.** `whitaker_lints`
  appears nowhere in the repository. Impact: corrected throughout this plan to
  `whitaker_suite`; ADR 002 must be corrected at EP-M5, before the string
  becomes copy-paste canon.

- **S-5 — `make verus` and `make kani` run in no CI workflow.** Only
  `scripts/check-verus-fragment-id-bridge.sh` does. Impact: contributed to
  D-11; a Verus obligation here would never have been enforced.

- **S-6 — no proof file in `verus/` has ever been modified.** All five are
  single "Add" commits, while `common/src` has taken 19 commits and
  `whitaker_clones_core/src` 8 over the same period. Impact: corroborates that
  a sidecar with no executable partner drifts silently.

## Decision log

- **D-1**: scope 1.3.1 to the macro crate alone, using the
  `#[whitaker_support_macros::dylint_expect(...)]` path in its own tests and
  documentation.
  Rationale: ADR 002 §Migration plan phase 1 covers exactly this crate, and
  roadmap 1.3.2 owns the facade. A placeholder facade now would be
  compatibility theatre — there is no consumer to be compatible with.
  Date/Author: 2026-08-21, planning agent.

- **D-2**: make the crate publish-ready and wire the release pipeline within
  1.3.1.
  Rationale: user direction, resolving ADR 002 §Outstanding decisions item 3.
  Amended by R-4: the new crate publishes **last**, not first, and every
  publish in the block is guarded against an already-uploaded version. The
  user's decision predates the discovery that `release.yml`'s publish block is
  all-or-nothing under `set -euxo pipefail`.
  Date/Author: 2026-08-21, user; amended by planning agent.

- **D-3**: move ADR 002 to `Accepted` within 1.3.1.
  Rationale: user direction. Now conditional on D-9: the ADR must first be
  **amended**, because accepting it unchanged would ratify an expansion that
  provably does not work.
  Date/Author: 2026-08-21, user; qualified by planning agent.

- **D-8**: migrate `crates/whitaker_test_macros` to the promoted workspace pins
  for `syn`, `quote`, and `proc-macro2`, with `full` added at its use site.
  Rationale: two pins for the same dependency is a second version of truth. The
  `full` feature stays local so it does not leak into the published manifest.
  Date/Author: 2026-08-21, planning agent.

- **D-9 — ARCHITECTURE DEVIATION. Status: ACCEPTED as option (b), 2026-08-21,
  by the repository owner.** ADR 002 has been amended: §Status, §Decision
  drivers, §Technical requirements, §Options considered (Option D and Table 1),
  §Decision outcome, §Known risks, and §Outstanding decisions. The macro emits
  the `cfg_attr` gate alone; the `check-cfg` entry is documented as the
  mechanism rather than a convenience. The original deviation record follows,
  retained for provenance.
  Affected upstream identifiers: ADR002-TR-1 (primary), ADR002-FR-4, and
  ADR 002 §Options considered, §Decision outcome, §Known risks.
  Finding: the mandated four-attribute expansion does not suppress
  `unexpected_cfgs` (S-1), two of its attributes suppress diagnostics that never
  fire (S-2), and the third removes the misspelt-lint safety net.
  Options:
  - **(a) Implement ADR 002 verbatim.** Roadmap-faithful. Ships a macro that
    does not achieve warning-freedom and that masks misspelt lint names.
    Requires no ADR amendment but knowingly delivers a broken requirement.
  - **(b) Amend ADR 002 and ship a minimal macro (recommended).** The
    expansion becomes the `cfg_attr` gate alone; Whitaker adds one `check-cfg`
    entry and documents it as the prerequisite for consumers. Preserves typo
    detection. Requires correcting §Options considered (Option D's rejection
    rationale is factually wrong), §Decision outcome, and §Known risks.
  - **(c) Supersede ADR 002 with Option D plus a lint.** No macro: one
    `check-cfg` line, plus a `dylint_expect_shape` lint that validates
    call-sites against loaded libraries and registered lint names — closing all
    three silent-no-op routes in R-2, which no macro can. On-thesis for a lint
    suite. Removes roadmap 1.3.1–1.3.4 as written.
  Recommendation: (b), with (c)'s lint booked as roadmap 1.3.5 regardless,
  since R-2's remaining two routes survive under every option.
  Required upstream change: ADR 002 amendment before EP-M1.
  Approving authority: repository owner.
  Date/Author: 2026-08-21, planning agent.

- **D-10**: defer the users' and developers' guide narratives to roadmap 1.3.4.
  Rationale: 1.3.4 is literally "Document intended usage, narrow-scope review
  guidance, and pre-expansion limitations". The earlier draft mapped
  ADR002-TR-4 into EP-M5, duplicating a later roadmap item. The same reasoning
  that correctly refuses to pre-empt 1.3.3 applies here. ADR 002, the suite
  design cross-reference, the repository layout, and the roadmap tick stay in
  EP-M5.
  Date/Author: 2026-08-21, planning agent, on reviewer finding.

- **D-11**: cut the Verus sidecar, the permutation property test, the BDD
  feature file, and the `googletest`/`pretty_assertions` dependencies.
  Rationale: the pigeonhole argument makes the 121-case enumeration a total
  decision procedure, so the Verus lemma would restate a decidable property —
  which `AGENTS.md` forbids. `make verus` runs in no CI workflow (S-5) and no
  proof file has ever been maintained (S-6). The seven BDD scenarios restate
  the `insta` snapshots with no stakeholder who reads `.feature` files but not
  `#[expect(...)]`, and `rstest-bdd-macros` is documented in
  `.config/nextest.toml` as hanging during dependency resolution on Windows CI.
  `googletest` and `pretty_assertions` appear in none of the repository's 20-plus
  crates; adding two assertion libraries for one crate forks the testing dialect
  and is its own decision, not a side-effect of this one. Net effect: three
  fewer verification layers, no loss of coverage.
  Date/Author: 2026-08-21, planning agent, on reviewer findings.

- **D-12**: hold `syn::LitStr` in `LibraryName` and `Reason` rather than
  `String`, and give `LibraryName` a validating constructor.
  Rationale: the earlier draft's newtypes carried no invariant and discarded the
  span. Rejecting an empty or hyphenated library name closes the one route to a
  silent no-op that the macro can close unaided, and keeping the span is what
  makes the diagnostics anchorable.
  Date/Author: 2026-08-21, planning agent, on reviewer finding.

- **D-13**: do not fix the aggregated-suite lint-level bug (R-1b) inside this
  plan.
  Rationale: the fix touches `suite/src/driver.rs` wiring, which `Constraints`
  places out of scope, and its blast radius covers every lint in the suite
  rather than anything this plan adds. It also needs its own regression
  coverage — a UI fixture per lint proving that `#[allow]` and `#[expect]`
  suppress under the aggregated library, which is exactly the coverage roadmap
  1.3.3 was scoped to build. Recorded in ADR 002 §Known risks, tracked as
  separate work, and gating ADR 002 migration phase 3 rather than 1.3.1
  delivery. Attempting it here would silently double the plan's scope and blur
  which change caused which regression.
  Date/Author: 2026-08-21, planning agent.

- **D-14**: keep `lib = "whitaker_suite"` as the canonical documented example
  despite R-1b.
  Rationale: it is the correct value for the shipping configuration, and will
  be correct once R-1b is fixed. Documenting the individual-library form
  instead would optimize the examples for a bug. R-1b is disclosed in ADR 002
  §Known risks so nobody adopts the attribute expecting it to work today.
  Date/Author: 2026-08-21, planning agent.

## Outcomes & retrospective

To be completed at EP-M5. Before setting this plan to `COMPLETE`, reconcile
every discovery against ADR 002:

- S-1, S-2, and S-4 require ADR 002 amendments and cannot be recorded as
  mechanical differences.
- R-2's surviving silent-no-op routes must appear in ADR 002 §Known risks and
  be booked as roadmap 1.3.5.
- R-3's deployment-mode problem must appear in ADR 002 §Outstanding decisions
  with the reserved `lib(...)` extension.
- S-3 must be raised as separate follow-up work against
  `crates/whitaker_test_macros`.

Do not mark this plan `COMPLETE` while any upstream change or deviation remains
unrecorded or unaccepted.

## Signposts

Documentation to read before starting:

- `docs/adr-002-dylint-expect-attribute-macro.md` — the governing decision,
  pending the D-9 amendment.
- `docs/whitaker-dylint-suite-design.md` — how the suite is assembled and where
  support crates sit.
- `docs/repository-layout.md` — the directory map.
- `docs/documentation-style-guide.md` — ADR section requirements, sentence-case
  headings, 80-column prose, table and figure captions, en-GB-oxendict spelling.
- `docs/developers-guide.md` §Creating a New Lint — relevant if D-9(c) is
  chosen, or for the 1.3.5 lint.
- `docs/rust-testing-with-rstest-fixtures.md` — fixture and parameterization
  conventions.
- `docs/rust-doctest-dry-guide.md` — keeping the rustdoc examples `AGENTS.md`
  requires from duplicating test logic.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` — the standard
  the 400-line and small-function rules serve.
- `AGENTS.md` — the binding style, testing, dependency, and commit rules.

Skills to load before starting:

- `leta` — semantic navigation; load at session start and prefer it to text
  search for symbol lookup.
- `rust-router` — routes to the narrower Rust skills; load first and follow.
- `arch-crate-design` — crate boundaries, `publish` decisions, and public
  versus internal API shape.
- `rust-unit-testing` — `rstest` parameterization and `insta` snapshot
  discipline.
- `proptest` — strategy design and shrinking for INV-EXP-3.
- `arch-decision-records` — for the ADR 002 amendment at EP-M5.
- `nextest` — filtersets for the focused commands, and R-8's group interaction.
- `execplans` — for keeping this document current.
- `commit-message` — file-based commit messages, never `-m`.

The `verus`, `kani`, and `hexagonal-architecture` skills were loaded during
planning and informed D-11 and the boundary discussion in `Interfaces and
dependencies`. They are **not** needed during implementation: there is no
proof obligation and no port to invert.

Sub-agents to use:

- `scrutineer` — the exclusive runner of full commit gates. Read its cited log
  rather than re-running a gate.
- `scribe` — the documentation edits at EP-M5.
- `wyvern` — read-only reconnaissance when a file's shape is unclear.
- `alchemist` — only for a single falsifiable hypothesis with a supplied
  prediction and minimal experiment.

## Revision note

**Revision 2, 2026-08-21.** Status moved from `DRAFT` to `BLOCKED`.

What changed. EP-M0's probes were run during planning rather than deferred, and
falsified a premise of ADR 002: the mandated expansion does not suppress
`unexpected_cfgs`, verified end-to-end with a real proc macro (S-1, probe 3e).
D-9 records the resulting proposed deviation with three options and a
recommendation. A six-lens design review then drove eleven further changes: the
verification apparatus was cut by three layers on a pigeonhole argument that
makes the enumeration total (D-11); the module layout was flattened after a
misread of `clippy::self_named_module_files`; `args/grammar` was renamed `keys`
because it never covered arity; `ArgShapeError` now carries position so
diagnostics can be anchored; the newtypes now hold `LitStr` and enforce an
invariant (D-12); `syn` lost the `full` feature that would have leaked to every
downstream consumer; the publish step was reordered and guarded (R-4); the
non-existent `whitaker_lints` library name was corrected throughout (S-4);
INV-DIAG-1's non-vacuity control was replaced because the original was itself
vacuous; the fixture inventory grew from ten to eighteen; and the guide
narratives were deferred to roadmap 1.3.4 (D-10).

Why. The plan's own prototyping milestone did its job. Acting on its findings
rather than proceeding around them is the point of having the milestone, and
the ExecPlan standard requires a deviation to be recorded and accepted rather
than absorbed.

Effect on remaining work. No implementation may begin until D-9 is resolved.
Under the recommended option (b) the plan is ready to execute as written; under
(a) the `Constraints` expansion clause and INV-EXP-2 revert to ADR 002's
four-attribute set; under (c) roadmap items 1.3.1–1.3.4 are superseded and this
plan is withdrawn. R-1 remains the one open empirical question under every
option, and must be answered before EP-M2 closes.

**Revision 3, 2026-08-21.** Status moved from `BLOCKED` to `APPROVED`.

What changed. The repository owner accepted deviation D-9 as option (b), so
ADR 002 was amended — §Status to `Accepted`, plus corrections to §Decision
drivers, §Technical requirements, §Options considered (Option D's rejection
rationale and two rows of Table 1), §Decision outcome, §Known risks, and
§Outstanding decisions — and the non-existent `whitaker_lints` library name was
corrected there too.

A spike then settled R-1, the last open empirical question, and was discarded
as instructed. `#[expect(...)]` **does** work for Dylint-registered Whitaker
lints, so axiom A-4 is established and R-1 is closed. The spike also surfaced
R-1b: the aggregated `whitaker_suite` library ignores lint-level attributes
entirely and emits spurious unfulfilled-expectation warnings, while individual
lint libraries built from the same commit behave correctly. Staleness,
lint-identity mismatch, and `cfg_attr` interaction were each ruled out by
controlled comparison.

Why it matters, and why it does not block. R-1b means no attribute-based
suppression works today in the configuration the macro targets, which is what
every installed consumer loads. It is disclosed in ADR 002 §Known risks and
tracked as D-13. It does not block 1.3.1, whose obligations are all
token-level, but it does gate ADR 002 migration phase 3 — adopting the
attribute across the estate before the fix would fill the estate with
annotations that suppress nothing.

Effect on remaining work. Implementation may begin at EP-M1. The one
outstanding EP-M0 item is committing the probes as a script with asserted exit
codes, so INV-WARN-1's negative control survives as a re-runnable artefact
rather than a transcript in this document.
