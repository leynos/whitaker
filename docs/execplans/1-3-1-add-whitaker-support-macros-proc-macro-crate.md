# Add the `whitaker_support_macros` proc-macro crate (roadmap 1.3.1)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`,
`Decision log`, `Outcomes & retrospective`, `Conformance basis`, and
`Verification plan` must be kept up to date as work proceeds.

Status: DRAFT

This document must be maintained in accordance with `AGENTS.md`. The canonical
plan file is
`docs/execplans/1-3-1-add-whitaker-support-macros-proc-macro-crate.md`.

## Purpose / big picture

Whitaker enforces project conventions through Dylint lint libraries. Dylint
lints are unknown to `rustc` during ordinary compilation, so writing
`#[expect(some_whitaker_lint)]` next to a deliberate exception produces
`unknown_lints` noise in every non-Dylint build. The documented workaround is a
four-attribute incantation that nobody remembers and that drifts between
call-sites.

After this change, a maintainer can write one attribute:

```rust,no_run
#[whitaker_support_macros::dylint_expect(
    lib = "whitaker_lints",
    lints(no_std_fs_operations),
    reason = "legacy call-site; remove once the cap-std migration lands"
)]
fn read_legacy_config() {}
```

and the expansion emits the full incantation, enabling `#[expect(...)]` only
when Dylint is actually running the named library. Because `expect` (rather
than `allow`) is used, the suppression announces itself the moment it becomes
stale.

Concretely, after this ExecPlan is complete:

1. `crates/whitaker_support_macros` exists as a `proc-macro = true` crate
   exporting one attribute macro, `dylint_expect`, accepting `lib = "..."`,
   `lints(path, ...)`, and an optional `reason = "..."`.
2. Applying the attribute to a function, method, `impl` block, module,
   `struct`, or trait compiles cleanly with warnings denied, under both
   `cargo check` and `cargo clippy`.
3. Malformed invocations produce precise, span-anchored compiler errors that
   name the offending argument, proven by `trybuild` compile-fail fixtures with
   byte-exact `.stderr` snapshots.
4. The argument grammar's well-formedness and order-independence are discharged
   by an unbounded Verus proof, an exhaustive enumeration test, and `proptest`
   permutation properties.
5. The crate is publish-ready and wired into the release pipeline.
6. ADR 002 moves from `Proposed` to `Accepted`, and the users' guide,
   developers' guide, repository layout, and Dylint suite design document all
   describe the new attribute.
7. `make check-fmt`, `make typecheck`, `make lint`, and `make test` all pass,
   and `docs/roadmap.md` item 1.3.1 is marked done.

Note the deliberate spelling of the attribute path during this milestone.
ADR 002 specifies the eventual call-site spelling as
`#[whitaker_support::dylint_expect(...)]`, but the `whitaker_support` facade
crate is roadmap item 1.3.2. Within 1.3.1 the macro is reached at
`#[whitaker_support_macros::dylint_expect(...)]`. This is not a compatibility
shim: it is the crate's own real path, and 1.3.2 adds the facade re-export
without changing anything delivered here.

## Context and orientation

Assume no prior knowledge of this repository.

### What Dylint is, and why this is hard

Dylint is a tool that loads out-of-tree lint libraries into the Rust compiler.
A Dylint lint such as `no_std_fs_operations` only exists while Dylint is
driving the compiler. During an ordinary `cargo check`, `rustc` has never heard
of it, so `#[expect(no_std_fs_operations)]` triggers the built-in
`unknown_lints` diagnostic.

Dylint's answer is conditional compilation. For every lint library it loads, it
passes `--cfg=dylint_lib="LIBRARY_NAME"` to `rustc`. Code can therefore write:

```rust,no_run
#[cfg_attr(dylint_lib = "whitaker_lints", expect(no_std_fs_operations))]
fn f() {}
```

The `expect` exists only when Dylint is running that library. Three further
diagnostics get in the way, and all three are denied in this workspace:

- `unexpected_cfgs` (root `Cargo.toml`, `[workspace.lints.rust]`, configured
  with `check-cfg = ['cfg(kani)']`) fires because `dylint_lib` is not an
  expected cfg key here.
- `clippy::allow_attributes` (root `Cargo.toml`, `[workspace.lints.clippy]`,
  level `deny`) fires on `#[allow(...)]` attributes, which the incantation
  needs.
- `clippy::allow_attributes_without_reason` (same stanza, level `deny`)
  requires every `allow` to carry a `reason = "..."`.

`unknown_lints` is also set to `deny` at `[workspace.lints.rust]`. Item-level
lint attributes override manifest levels, so an item-scoped
`#[allow(unknown_lints, reason = "...")]` still works.

The macro's job is to emit all of this correctly, once, from one legible
call-site.

### Key files a newcomer needs

- `docs/adr-002-dylint-expect-attribute-macro.md` — the governing decision.
  Read §Decision outcome / proposed direction and §Functional requirements
  before writing any code. It specifies the crate name, the argument shape, and
  the exact set of attributes the expansion must emit.
- `docs/roadmap.md` lines 28–47 — the 1.3.x task group. 1.3.1 is this plan;
  1.3.2 adds the `whitaker_support` facade; 1.3.3 adds cross-configuration
  compatibility coverage; 1.3.4 completes the narrative documentation.
- `crates/whitaker_test_macros/` — the only existing `proc-macro = true` crate
  in the workspace, and the manifest precedent to follow. It is 32 lines, has
  no tests of its own, and ignores its attribute arguments entirely, so it
  offers no precedent for argument parsing or diagnostics.
- `Cargo.toml` (root) — `members = ["common", "crates/*", "installer",
  "suite"]`. A new directory under `crates/` joins the workspace automatically;
  no `members` edit is required. `[workspace.dependencies]` (lines 12–66) is
  where shared version pins live. `[workspace.lints.*]` (lines 105 onward) is
  the lint policy the expansion must satisfy.
- `Makefile` — `WHITAKER_PACKAGES` (line 81) enumerates the crates the Whitaker
  Dylint suite lints. `typecheck` (223–224), `lint-clippy` (179–181), and
  `test` (92–139) all operate workspace-wide and need no per-crate edit.
- `.github/workflows/ci.yml` line 160 and `.github/workflows/release.yml`
  lines 334–339 — the two places crates are enumerated for packaging and
  publishing.
- `verus/` and `scripts/run-verus.sh` — the Verus proof sidecar. Proof files
  live at the repository root under `verus/`, outside the Cargo workspace, and
  are grouped by `proof_files_for_group` in the script.
- `rust-toolchain.toml` — pinned to `nightly-2026-05-28`.

### Terms defined

- **Attribute macro**: a procedural macro invoked as `#[name(args)] item`. It
  receives the argument tokens and the item tokens, and returns replacement
  tokens.
- **Pre-expansion lint**: a lint that runs before macro expansion. `cfg_attr`
  gating cannot help such a lint, because the gating has not been applied when
  the lint runs. ADR 002 §Known risks names this as an accepted limitation.
- **`expect` versus `allow`**: `#[allow(L)]` silences `L` forever.
  `#[expect(L)]` silences `L` but emits `unfulfilled_lint_expectations` if `L`
  never fires, so stale suppressions surface.
- **Sidecar proof**: a Verus source file under `verus/` that models repository
  logic. Verus compiles its own files and cannot `use` crate modules, so the
  model is kept in step with the implementation by review.

## Conformance basis

- Governing decision record: `docs/adr-002-dylint-expect-attribute-macro.md`,
  as at commit `02e6c1c` (status `Proposed` at plan authoring time; this plan
  moves it to `Accepted`).
- Roadmap: `docs/roadmap.md` §1.3, item 1.3.1.
- Governing standards: `AGENTS.md`, `docs/documentation-style-guide.md`,
  `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rust-doctest-dry-guide.md`, `docs/rstest-bdd-users-guide.md`,
  `docs/complexity-antipatterns-and-refactoring-strategies.md`.
- There is no Terms of Reference artefact for this work. ADR 002 is the sole
  upstream requirements source; do not invent a higher-level document.

Traced requirement identifiers are taken from ADR 002 §Functional requirements
and §Technical requirements, which are unnumbered in the source. This plan
assigns stable local identifiers and states the source sentence for each, so
the mapping is auditable.

| Identifier    | ADR 002 source                                                             |
| ------------- | -------------------------------------------------------------------------- |
| ADR002-FR-1   | "Provide an item attribute usable on functions, impl blocks, modules, and other Rust items." |
| ADR002-FR-2   | "Support one or multiple lint names per annotation."                       |
| ADR002-FR-3   | "Support an optional human-readable reason."                               |
| ADR002-FR-4   | "Enable `#[expect(...)]` only when Dylint runs the specified lint library." |
| ADR002-TR-1   | "Avoid warnings in non-Dylint builds" (`unknown_lints`, `unexpected_cfgs`, `clippy::allow_attributes`). |
| ADR002-TR-2   | "Keep the macro's expansion explicit and reviewable."                      |
| ADR002-TR-3   | "Maintain a clear separation between proc-macro code and lint implementation code." |
| ADR002-TR-4   | "Document limitations for 'pre-expansion' lints."                          |
| ADR002-MIG-1  | §Migration plan phase 1: "Add `crates/whitaker_support_macros` with the proc-macro implementation." |

_Table 1: Local identifiers assigned to ADR 002's unnumbered requirements._

Trace chains:

```plaintext
ADR002-FR-1 -> EP-M3 -> crates/whitaker_support_macros/tests/applies_to_items.rs
ADR002-FR-2 -> EP-M2 -> expand::tests::preserves_lint_order_and_multiplicity
ADR002-FR-3 -> EP-M2 -> expand::tests::reason_is_propagated_into_expect
ADR002-FR-4 -> EP-M2 -> snapshots/dylint_expect@cfg_attr_gate.snap
ADR002-TR-1 -> EP-M3 -> make lint (clippy, warnings denied) over the new crate
ADR002-TR-2 -> EP-M2 -> insta snapshots of every expansion variant
ADR002-TR-3 -> EP-M1 -> crate layout review: only src/lib.rs names `proc_macro`
ADR002-TR-4 -> EP-M5 -> docs/users-guide.md, docs/developers-guide.md
ADR002-MIG-1 -> EP-M1 -> cargo metadata lists whitaker_support_macros
INV-SHAPE-1 -> EP-M2 -> verus/whitaker_support_dylint_expect_args.rs
INV-SHAPE-2 -> EP-M2 -> args::grammar::tests::exhaustive_key_sequences_to_length_four
```

## Constraints

These are hard invariants. Violation requires escalation, not a workaround.

- The macro's public argument surface is fixed by ADR 002: `lib = "..."`
  (string literal), `lints(path, ...)` (one or more), optional
  `reason = "..."`. Do not add, rename, or reorder these.
- The expansion must emit exactly the attribute set ADR 002 §Decision outcome
  lists: `allow(clippy::allow_attributes)`, `allow(unknown_lints)`,
  `allow(unexpected_cfgs)`, and `cfg_attr(dylint_lib = "...", expect(...))`.
  Adding `reason = "..."` to the three `allow` attributes is a required
  addition (see `Decision log`, D-4); adding anything else is not.
- Only `crates/whitaker_support_macros/src/lib.rs` may name the `proc_macro`
  crate. Every other module operates on `proc_macro2` and `syn` types so that
  it is unit-testable outside a compiler invocation. This is the ADR002-TR-3
  boundary.
- No file may exceed 400 lines (`AGENTS.md`).
- The `mod.rs` module-file style is required: `clippy::self_named_module_files`
  is denied workspace-wide, so a directory module `args/` must contain
  `args/mod.rs`, never `args.rs` beside it.
- `clippy::unwrap_used`, `clippy::expect_used`, `clippy::indexing_slicing`, and
  `clippy::panic_in_result_fn` are denied. Proc-macro code must return
  `syn::Result` and convert to `compile_error!` at the single adapter boundary.
- `unsafe_code` is forbidden; `missing_docs` and
  `rustdoc::missing_crate_level_docs` are denied.
- Do not modify any existing lint crate, `common/`, `suite/`, `installer/`, or
  `src/`. This plan adds one crate and edits manifests, workflows, the Verus
  sidecar, and documentation only.
- Comments and documentation use en-GB-oxendict spelling. Markdown prose wraps
  at 80 columns; code blocks at 120.
- Do not mutate the parent process environment in tests. Where a test needs a
  different environment, set it on a child process.
- Use the shared default Cargo cache. Do not create an isolated `CARGO_HOME`.

## Tolerances (exception triggers)

- Scope: if implementation requires touching more than 25 files or more than
  1200 net lines, stop and escalate.
- Interface: if the ADR 002 argument surface or the mandated expansion set must
  change, stop and escalate — that is an ADR amendment, not an implementation
  detail.
- Dependencies: the plan already adds `syn`, `quote`, `proc-macro2`,
  `googletest`, and `pretty_assertions`. Any further new external dependency
  requires escalation.
- Verification: if the Verus proof for INV-SHAPE-1 cannot be discharged within
  6 attempts, stop and escalate with the failing goal, rather than weakening the
  lemma.
- Iterations: if `make check-fmt`, `make typecheck`, `make lint`, or `make test`
  still fail after 3 targeted fix attempts on the same milestone, stop and
  escalate with the `tee`'d log path.
- Prototype outcome: if EP-M0 shows that `#[expect(...)]` cannot work for
  Dylint lints at all, stop and escalate. That falsifies ADR 002 §Decision
  outcome and requires an ADR amendment before any further work.
- Ambiguity: if two readings of ADR 002 lead to materially different
  expansions, present both with trade-offs rather than choosing silently.

## Risks

- Risk: `#[expect(LINT)]` may not work for Dylint-registered lints. Dylint's
  own documentation consistently demonstrates `allow`, never `expect`, and a
  documentation query returned no evidence that `unfulfilled_lint_expectations`
  fires correctly for out-of-tree lints. ADR 002's entire value proposition
  depends on `expect` working.
  Severity: high. Likelihood: medium.
  Mitigation: EP-M0 is a prototyping milestone that answers this empirically
  before any production code is written. If `expect` does not work, escalate;
  do not silently substitute `allow`.

- Risk: `#[allow(clippy::allow_attributes, reason = "...")]` may not suppress
  `clippy::allow_attributes` on its own attribute list, producing a
  self-referential warning that makes the expansion un-lintable.
  Severity: high. Likelihood: medium.
  Mitigation: EP-M0 probes this directly. Contingency, in preference order:
  (a) reorder so the suppressing attribute precedes the others; (b) collapse
  the three `allow` attributes into one; (c) escalate, because switching to
  `expect(clippy::allow_attributes)` changes the ADR-mandated expansion set.

- Risk: emitting `#[allow(unknown_lints)]` unconditionally masks a genuine
  typo. If a maintainer misspells a lint name, the `expect` silently does
  nothing and the `unknown_lints` diagnostic that would have caught it is
  suppressed. ADR 002 §Known risks names the adjacent `lib`-mismatch case but
  not this one.
  Severity: medium. Likelihood: medium.
  Mitigation: ADR 002 mandates the attribute, so emit it. Document the hazard
  in the users' guide and developers' guide as part of EP-M5, and record it in
  ADR 002 §Known risks when moving the status to `Accepted`. Note that the
  `unfulfilled_lint_expectations` diagnostic does not rescue this case, because
  a misspelled lint never registers an expectation.

- Risk: Kani cannot compile a `proc-macro = true` crate, removing bounded model
  checking as an option for the in-crate grammar policy.
  Severity: low. Likelihood: high.
  Mitigation: EP-M0 confirms this with a one-command probe. The fallback is
  strictly stronger for this obligation than a bounded Kani run would be: the
  argument-key alphabet has three symbols, so an exhaustive enumeration test
  over all sequences to length 4 (121 cases) is a complete decision procedure
  within that bound, and the Verus proof covers the unbounded case. See
  `Verification plan`, D-6.

- Risk: publishing wiring lands before the `whitaker_support` facade exists, so
  the first published version of `whitaker_support_macros` has no companion.
  Severity: low. Likelihood: high.
  Mitigation: this is intended and harmless — the macro crate is usable on its
  own at its own path. `release.yml` must publish `whitaker_support_macros`
  before any future crate that depends on it; order the step accordingly now.

- Risk: `cargo package` verification fails because a test fixture confuses the
  packaging build.
  Severity: low. Likelihood: low.
  Mitigation: no nested `Cargo.toml` is committed anywhere under the new crate.
  EP-M4 runs `make publish-check PUBLISH_PACKAGES="whitaker_support_macros"`
  explicitly to prove packaging works.

- Risk: `insta` snapshots of token streams are brittle against `prettyplease`
  or `quote` formatting changes.
  Severity: low. Likelihood: low.
  Mitigation: snapshot the parsed-and-reprinted attribute list via a stable
  normalizer rather than raw `TokenStream::to_string()` output, and pin the
  normalizer in one helper so a formatting change is a one-line re-bless.

## Verification plan

The implementation is decomposed specifically so that the interesting
obligations land on a pure function over a three-symbol alphabet, rather than
on token trees. That decomposition is the point: token-tree properties are
verified by example and snapshot, and the one genuinely general property is
verified by proof.

### Axioms (assumed, not verified)

- A-1: `syn` 2.x parses `lib = "..."`, `lints(a, b)`, and `reason = "..."`
  argument forms into the token structures its documentation describes. Third-
  party internals are not verified. Repository-owned logic built on this
  interface is verified against the real `syn` parser in unit tests.
- A-2: `rustc` applies item-level lint-level attributes in preference to
  manifest-level `[lints]` configuration. Exercised concretely by EP-M0 and by
  every warning-free compile in `make lint`.
- A-3: Dylint passes `--cfg=dylint_lib="LIBRARY_NAME"` for each loaded library,
  with that exact key and value form. Sourced from Dylint's documentation;
  ADR 002 §Known risks already records the mismatch hazard. This axiom is
  discharged empirically at roadmap 1.3.3, not here.
- A-4: `#[expect(L)]` for a lint `L` registered by a Dylint library behaves as
  `expect` does for built-in lints. **This axiom is not assumed — EP-M0
  establishes it empirically, because ADR 002 depends on it and the evidence
  for it is currently absent.**

### Obligation INV-SHAPE-1: argument validation is order-independent

- Obligation: for any two sequences of supplied argument keys `a` and `b` with
  equal multisets, `validate_key_sequence(a)` succeeds exactly when
  `validate_key_sequence(b)` succeeds.
- Method: Verus deductive proof over `Seq<ArgKey>`, plus a `proptest`
  permutation property against the real Rust implementation.
- Rationale: the property quantifies over unbounded sequences and over all
  permutations, which no finite test can establish. It is also the property a
  naive implementation most plausibly breaks: an accumulator-based parser that
  records "the last `lib` wins" satisfies every single-order example test while
  violating order-independence for duplicate inputs. The Verus proof discharges
  the model; the `proptest` property ties the Rust implementation to it.
- Domain: `Seq<ArgKey>` for `ArgKey ∈ {Lib, Lints, Reason}`, unbounded length
  in Verus; generated sequences of length 0–8 with generated permutations in
  `proptest`.
- Artefact: `verus/whitaker_support_dylint_expect_args.rs`;
  `crates/whitaker_support_macros/src/args/grammar/tests.rs`.
- Evidence: `make verus` (after adding a `support-macros` group to
  `scripts/run-verus.sh`) reports `verification results:: N verified, 0 errors`.
  Before the implementation exists the proof file does not compile, which is the
  red state. `cargo nextest run -p whitaker_support_macros` runs the property.
- Non-vacuity: the proof's antecedent `a.to_multiset() == b.to_multiset()` is
  inhabited by `a = [Lib, Lints]`, `b = [Lints, Lib]`, and the `ensures` is
  non-trivial because `validate` is modelled as a left fold with an
  accumulator, not as a multiset predicate — the lemma has to bridge the two.
  Negative control: temporarily remove the duplicate-key guard from `validate`
  in the Verus model and confirm `lemma_validate_matches_shape` fails; remove
  it from the Rust implementation and confirm the `proptest` property shrinks
  to a `[Lib, Lib, Lints]`-shaped counterexample. Both mutations must be
  reverted before the milestone closes.

### Obligation INV-SHAPE-2: validation accepts exactly the well-shaped sequences

- Obligation: `validate_key_sequence(keys)` succeeds if and only if `keys`
  contains exactly one `Lib`, exactly one `Lints`, and at most one `Reason`.
- Method: Verus proof (`lemma_validate_matches_shape`) for the unbounded case,
  plus an exhaustive enumeration test over every sequence of length 0–4.
- Rationale: the "if and only if" is what makes the error taxonomy trustworthy;
  a one-directional test suite would pass against an implementation that
  rejects valid inputs. Exhaustive enumeration over a three-symbol alphabet to
  length 4 is 1 + 3 + 9 + 27 + 81 = 121 cases and is a complete decision
  procedure within that bound — strictly more than a bounded model checker
  would give, at negligible cost. See `Decision log` D-6 for why Kani is not
  used.
- Domain: all `s ∈ {Lib, Lints, Reason}*` with `|s| ≤ 4` exhaustively;
  unbounded in Verus.
- Artefact: `crates/whitaker_support_macros/src/args/grammar/tests.rs`,
  test `exhaustive_key_sequences_to_length_four`;
  `verus/whitaker_support_dylint_expect_args.rs`.
- Evidence: `cargo nextest run -p whitaker_support_macros -E
  'test(exhaustive_key_sequences)'` reports 1 passed. The test fails before
  `validate_key_sequence` exists (compile error) and after the seeded mutation.
- Non-vacuity: the enumeration includes the empty sequence (must be rejected
  for `MissingLib`), every singleton (all rejected), the two accepting
  two-element orders, both three-element accepting orders with `Reason`, and
  every duplicate-bearing sequence. The test asserts the *specific* error
  variant, not merely that an error occurred, so an implementation that
  collapses all failures into one variant is rejected. Negative control:
  changing `MissingLints` to `MissingLib` in one branch must fail the test.

### Obligation INV-EXP-1: the annotated item is preserved verbatim

- Obligation: the expansion's token stream ends with exactly the input item
  tokens, with nothing inserted, removed, or reordered inside them.
- Method: parameterized `rstest` cases comparing the expansion's item suffix
  against the input, across item kinds; plus compile-level evidence from
  `tests/applies_to_items.rs`.
- Rationale: this is a finite partition over Rust item kinds, so parameterized
  cases are proportionate. A property test over arbitrary token trees would
  test `quote`'s interpolation, which is a third-party internal (A-1).
- Domain: `fn`, `fn` with generics and where-clause, `impl` block, inherent
  method, `mod`, `struct`, `trait`, and an item that already carries other
  attributes and doc comments.
- Artefact: `crates/whitaker_support_macros/src/expand/tests.rs`;
  `crates/whitaker_support_macros/tests/applies_to_items.rs`.
- Evidence: `cargo nextest run -p whitaker_support_macros`. Red state: the
  cases do not compile before `expand` exists.
- Non-vacuity: the "item already carries attributes and doc comments" case is
  the one that fails if the implementation re-parses and re-emits the item
  rather than passing tokens through. Negative control: make `expand` drop the
  item's existing attributes and confirm that case fails.

### Obligation INV-EXP-2: the mandated attribute set is emitted, in order

- Obligation: the expansion emits exactly the four ADR 002 attributes, in the
  documented order, before the item.
- Method: `insta` snapshot tests over the normalized attribute prelude, one
  snapshot per variant.
- Rationale: ADR002-TR-2 requires the expansion be "explicit and reviewable".
  Snapshots make every change to the emitted attributes visible in review,
  which is precisely the multivariant output-format-consistency case snapshots
  exist for.
- Domain: single lint; multiple lints; with reason; without reason;
  tool-qualified lint path (`clippy::needless_return`); library name containing
  underscores.
- Artefact: `crates/whitaker_support_macros/src/expand/tests.rs` with snapshots
  under `crates/whitaker_support_macros/src/snapshots/`.
- Evidence: `cargo nextest run -p whitaker_support_macros` with
  `INSTA_UPDATE=no`; new snapshots are unreviewed (red) until blessed.
- Non-vacuity: the six variants differ from one another in the snapshot output,
  so a implementation that ignores `reason` or flattens the lint list produces
  identical snapshots for distinct inputs, which review catches. Negative
  control: drop `allow(unexpected_cfgs)` from the expansion and confirm all six
  snapshots fail.

### Obligation INV-EXP-3: lint order and multiplicity are preserved

- Obligation: the lint paths inside `expect(...)` are exactly the paths given
  to `lints(...)`, in the same order, with the same multiplicity.
- Method: `proptest` property over generated lists of lint paths.
- Rationale: this is an invariant over a range of inputs — arbitrary-length
  lists of arbitrary identifiers — so a property test is the proportionate
  choice. Diagnostics quality depends on it: silently deduplicating or sorting
  lints would make the expansion diverge from the call-site a reviewer reads.
- Domain: generated `Vec<Path>` of length 1–8 drawn from a pool of identifier
  and `tool::lint` shapes, including deliberate repeats.
- Artefact: `crates/whitaker_support_macros/src/expand/tests.rs`.
- Evidence: `cargo nextest run -p whitaker_support_macros`; regression seeds
  under `crates/whitaker_support_macros/proptest-regressions/`.
- Non-vacuity: the generator's pool deliberately contains repeats, so the
  "same multiplicity" clause is reachable; record `proptest` classification
  showing at least 20% of generated cases contain a repeat, and treat a lower
  rate as a generator defect. Negative control: insert a `.dedup()` into the
  expansion and confirm the property shrinks to a two-element repeated list.

### Obligation INV-DIAG-1: malformed invocations produce specific diagnostics

- Obligation: each malformed-argument class produces a distinct, span-anchored
  compiler error naming the offending argument.
- Method: `trybuild` compile-fail fixtures with byte-exact `.stderr` snapshots.
- Rationale: diagnostic text and span placement are only observable through a
  real compilation. This is the standard, and only faithful, boundary test for
  proc-macro error reporting.
- Domain: missing `lib`; missing `lints`; empty `lints()`; duplicate `lib`;
  duplicate `lints`; duplicate `reason`; non-string-literal `lib`;
  non-string-literal `reason`; unknown argument key; empty argument list.
- Artefact: `crates/whitaker_support_macros/tests/ui.rs` with fixtures under
  `crates/whitaker_support_macros/tests/ui/`.
- Evidence: `cargo nextest run -p whitaker_support_macros -E
  'binary(ui)'`. Each fixture fails with no `.stderr` present (red), then
  passes once the `.stderr` is blessed from a reviewed `wip` file.
- Non-vacuity: the ten fixtures must produce ten distinct `.stderr` contents; a
  review step compares them and treats any two identical files as a failure,
  because that means the implementation collapsed two error classes. Negative
  control: replace the duplicate-key error message with the missing-key message
  and confirm two `.stderr` files then mismatch.

### Obligation INV-WARN-1: the expansion is warning-free in a non-Dylint build

- Obligation: applying the attribute to any supported item kind produces no
  diagnostic under `cargo check` or `cargo clippy` with warnings denied and
  this workspace's full lint policy in force.
- Method: compile-level evidence from the repository's own gates, applied to
  real usages of the macro inside the new crate.
- Rationale: this is ADR002-TR-1, and the honest way to check it is to
  actually compile code that uses the macro under the exact lint configuration
  the workspace enforces. `crates/whitaker_support_macros/tests/*.rs` and the
  crate's doctests are compiled under `RUSTFLAGS="-D warnings"` by `make test`
  and by `cargo clippy ... -- -D warnings` in `make lint-clippy`, and the crate
  inherits `[lints] workspace = true`. No bespoke harness is needed.
- Domain: the non-Dylint configuration only. The Clippy-run and Dylint-run
  configurations across a matrix are roadmap 1.3.3's scope; do not build that
  harness here.
- Artefact: `crates/whitaker_support_macros/tests/applies_to_items.rs`, the
  crate's rustdoc examples, and the `make lint` / `make test` gates.
- Evidence: `make lint 2>&1 | tee /tmp/lint-whitaker-1-3-1.out` exits 0 with no
  warning lines mentioning `whitaker_support_macros`.
- Non-vacuity: EP-M0 establishes that the naive expansion *does* warn under
  this configuration, so the gate is known to be capable of failing. Record
  that EP-M0 transcript as the negative control; without it, a clean `make
  lint` proves nothing.

### Obligations deliberately not taken

No unbounded lemma is introduced by the expansion logic itself: `expand` is a
straight-line render of a validated struct into a token stream, and its
correctness is fully characterized by the finite variant matrix in INV-EXP-2
together with the pass-through property in INV-EXP-1. Stating a Verus lemma
over a hand-written model of `TokenStream` would restate the rendering code in
a second notation and prove that the two agree, which is the vacuous pattern
the ExecPlan standard forbids. The genuine general property in this change is
the argument grammar, and it is proved.

## Plan of work

### Stage A — prototype and de-risk (no production code)

Answer the three empirical questions the design rests on, in a scratch
directory outside the repository, then delete the scratch work. Nothing in this
stage is committed except the recorded findings.

1. Does `#[expect(L)]` work for a Dylint-registered lint, and does
   `unfulfilled_lint_expectations` fire when `L` stops triggering? Build one of
   the existing lint crates (for example `no_std_fs_operations`) as a Dylint
   library and run it over a two-file fixture: one where the lint fires under an
   `expect`, one where it does not.
2. Does `#[allow(clippy::allow_attributes, reason = "...")]` suppress
   `clippy::allow_attributes` for the sibling `allow` attributes on the same
   item, and for itself? Test under `cargo clippy -- -D
   clippy::allow_attributes -D clippy::allow_attributes_without_reason`.
3. Record the *unmitigated* diagnostic output: compile the naive
   `#[cfg_attr(dylint_lib = "x", expect(y))] fn f() {}` with no `allow`
   attributes, under this workspace's lint policy, and capture the warnings.
   This transcript is the negative control for INV-WARN-1.
4. Confirm whether `cargo kani` can compile a `proc-macro = true` crate.

Go/no-go: if (1) shows `expect` does not work for Dylint lints, stop and
escalate — ADR 002 needs amending. If (2) shows self-suppression fails, apply
the ordering or collapsing contingency from `Risks` and re-probe; escalate only
if neither works.

### Stage B — red tests and the specification

Create the crate skeleton and every failing test before any logic exists.

1. `crates/whitaker_support_macros/Cargo.toml` with `[lib] proc-macro = true`,
   full publish metadata, and dev-dependencies.
2. `crates/whitaker_support_macros/src/lib.rs` containing only the crate-level
   `//!` documentation, the module declarations, and a `dylint_expect` entry
   point that returns `compile_error!("not yet implemented")`.
3. The feature specification at
   `crates/whitaker_support_macros/tests/features/dylint_expect.feature`,
   quoted in full under `Artefacts and notes` below.
4. All unit, property, exhaustive, snapshot, and BDD tests, written against
   the intended API. They will not compile — that is the red state, and it is
   the expected failure reason.
5. All ten `trybuild` compile-fail fixtures with no `.stderr` files.
6. `verus/whitaker_support_dylint_expect_args.rs` containing the `ArgKey` spec
   type, `count_of`, `is_well_shaped`, `validate`, and the two lemma signatures
   with `assume(false)` placeholders, so `make verus` reports the goals as open.

Validation for Stage B: `cargo nextest run -p whitaker_support_macros` fails to
compile with errors naming the missing items. Record the transcript.

### Stage C — implementation and proof, developed together

Work in this order, because each step's tests are already red:

1. `src/args/grammar/mod.rs` — the `ArgKey` and `ArgShapeError` enums and
   `validate_key_sequence`. This is the pure policy, and the only module the
   Verus model mirrors. Turn `exhaustive_key_sequences_to_length_four` green.
2. `verus/whitaker_support_dylint_expect_args.rs` — replace the `assume(false)`
   placeholders with real proofs. Discharge INV-SHAPE-1 and INV-SHAPE-2. Run
   the seeded-mutation negative controls and revert them.
3. `src/args/parse.rs` — the `syn::parse::Parse` implementation that maps
   tokens to `(ArgKey, payload)` pairs, calls `validate_key_sequence`, then
   assembles `DylintExpect`. Turn the parsing unit tests green.
4. `src/expand/mod.rs` — the renderer. Turn INV-EXP-1, INV-EXP-2, and
   INV-EXP-3 green, and bless the snapshots after reviewing each one.
5. `src/lib.rs` — wire the adapter: parse, expand, and convert
   `syn::Error` to `to_compile_error()` on the failure path. Bless the
   `trybuild` `.stderr` fixtures after reviewing each for span placement and
   wording, and confirm all ten differ.

Validation for Stage C: `cargo nextest run -p whitaker_support_macros` passes;
`make verus` reports zero errors.

### Stage D — wiring, documentation, and wider validation

1. Root `Cargo.toml`: add `syn`, `quote`, `proc-macro2`, `googletest`, and
   `pretty_assertions` to `[workspace.dependencies]`, and add the
   `whitaker_support_macros` path-and-version entry.
2. `Makefile`: append `-p whitaker_support_macros` to `WHITAKER_PACKAGES`.
3. `scripts/run-verus.sh`: add a `support-macros` group and include the new
   proof file in the `all` group.
4. `.github/workflows/ci.yml` line 160: add `whitaker_support_macros` to
   `PUBLISH_PACKAGES`.
5. `.github/workflows/release.yml`: add `cargo publish -p
   whitaker_support_macros` as the *first* publish step.
6. Documentation, in this order: ADR 002 status and known-risks update;
   `docs/whitaker-dylint-suite-design.md` cross-reference;
   `docs/repository-layout.md` support-crate bullet;
   `docs/developers-guide.md` convention section;
   `docs/users-guide.md` suppression section; `docs/roadmap.md` 1.3.1 to `[x]`.
7. Run the full gate set.

Validation for Stage D: `make check-fmt`, `make typecheck`, `make lint`,
`make test`, `make markdownlint`, `make nixie`, and `make verus` all pass.

## Milestones and plateaus

### EP-M0 — prototype findings recorded (prototyping milestone)

- Outcome: the four Stage A questions are answered with transcripts, recorded
  in `Surprises & discoveries` and `Decision log`. No repository files change
  except this plan.
- Requirements and gaps: de-risks ADR002-FR-4 and ADR002-TR-1; establishes
  axiom A-4.
- Acceptance evidence: EV-M0, a transcript in `Artefacts and notes` showing the
  `expect` behaviour, the Clippy self-suppression behaviour, the unmitigated
  warning output, and the Kani result.
- Conformance check: does the evidence still support ADR 002 §Decision
  outcome? If not, set status `BLOCKED` and propose an ADR amendment.
- Recovery: the scratch directory is outside the repository; delete it.
- Remaining gaps: everything.
- Compatibility decision: none required.

### EP-M1 — crate skeleton exists and the workspace still builds

- Outcome: `crates/whitaker_support_macros` is a workspace member with correct
  metadata and lint inheritance; `cargo metadata` lists it; `make typecheck`
  passes. The macro is a stub that always errors.
- Requirements and gaps: ADR002-MIG-1, ADR002-TR-3.
- Acceptance evidence: EV-M1 — `cargo metadata --format-version 1 --no-deps |
  jq -r '.packages[].name' | grep whitaker_support_macros` prints the name, and
  `make typecheck` exits 0.
- Conformance check: only `src/lib.rs` names `proc_macro`; `publish` metadata
  matches the crates.io decision; no new dependency beyond those approved.
- Recovery: `git checkout -- crates/whitaker_support_macros` and remove the
  directory; the `crates/*` glob makes removal complete.
- Remaining gaps: all behaviour.
- Compatibility decision: none. The crate is new and has no consumers.

### EP-M2 — the grammar and expansion are correct and proved

- Outcome: `dylint_expect` parses, validates, and expands correctly; every
  unit, exhaustive, property, snapshot, and BDD test passes; the Verus proof
  discharges INV-SHAPE-1 and INV-SHAPE-2; `trybuild` fixtures are blessed.
- Requirements and gaps: ADR002-FR-1 through ADR002-FR-4, ADR002-TR-2;
  INV-SHAPE-1, INV-SHAPE-2, INV-EXP-1, INV-EXP-2, INV-EXP-3, INV-DIAG-1.
- Acceptance evidence: EV-M2 — `cargo nextest run -p whitaker_support_macros`
  reports all tests passed with the count recorded, and `make verus` reports
  `0 errors`. Both negative-control mutations have been run and reverted, with
  their failing output recorded.
- Conformance check: the emitted attribute set matches ADR 002 §Decision
  outcome exactly, apart from the D-4 reason additions; no file exceeds 400
  lines; no `unwrap`/`expect` in non-test code.
- Recovery: snapshots and `.stderr` fixtures are regenerable with
  `INSTA_UPDATE=always` and `TRYBUILD=overwrite`, but must be re-reviewed
  before committing, never blessed blind.
- Remaining gaps: workspace wiring, release wiring, documentation.
- Compatibility decision: none.

### EP-M3 — the crate is warning-free under the full workspace lint policy

- Outcome: `make lint` and `make test` pass with the new crate included, and
  the Whitaker Dylint suite lints it via `WHITAKER_PACKAGES`.
- Requirements and gaps: ADR002-TR-1; INV-WARN-1.
- Acceptance evidence: EV-M3 — `make lint 2>&1 | tee
  /tmp/lint-whitaker-1-3-1.out` exits 0 with no warnings referencing the new
  crate, contrasted against the EP-M0 unmitigated transcript.
- Conformance check: if adding the crate to `WHITAKER_PACKAGES` breaks the
  Dylint check build (as it may for proc-macro crates), record the failure,
  revert that one line, and note the deviation in `Decision log` rather than
  weakening any lint.
- Recovery: revert the `Makefile` line; the rest of the milestone stands.
- Remaining gaps: release wiring, documentation.
- Compatibility decision: none.

### EP-M4 — the crate is publish-ready and wired into release

- Outcome: `make publish-check PUBLISH_PACKAGES="whitaker_support_macros"`
  succeeds; `ci.yml` and `release.yml` include the crate.
- Requirements and gaps: resolves ADR 002 §Outstanding decisions item 3.
- Acceptance evidence: EV-M4 — the `cargo package` step completes and the
  produced `.crate` file lists `src/`, `Cargo.toml`, and the licence, with no
  nested manifest.
- Conformance check: `cargo publish -p whitaker_support_macros` is ordered
  before any dependent crate's publish step; all dependencies are published
  crates with caret version requirements.
- Recovery: revert the two workflow edits; nothing has been published, because
  publishing only happens on a release tag.
- Remaining gaps: documentation.
- Compatibility decision: none. First publication of a new crate.

### EP-M5 — documentation is complete and the roadmap is updated

- Outcome: ADR 002 is `Accepted` with a dated summary and an expanded
  §Known risks; the suite design, repository layout, developers' guide, and
  users' guide describe the attribute; `docs/roadmap.md` 1.3.1 is `[x]`.
- Requirements and gaps: ADR002-TR-4.
- Acceptance evidence: EV-M5 — `make markdownlint` and `make nixie` pass, and
  `rg -n 'dylint_expect' docs/` lists ADR 002, the suite design, the
  developers' guide, the users' guide, the repository layout, and this plan.
- Conformance check: reconcile every EP-M0 discovery against ADR 002. The
  `unknown_lints` masking hazard and the `expect`-viability finding must both
  appear in ADR 002 §Known risks before the status moves to `Accepted`.
- Recovery: documentation edits are independently revertible.
- Remaining gaps: roadmap items 1.3.2, 1.3.3, and 1.3.4 remain open by design.
- Compatibility decision: none.

## Interfaces and dependencies

### Crate layout

```plaintext
crates/whitaker_support_macros/
├── Cargo.toml
├── src/
│   ├── lib.rs                  # adapter: the ONLY file naming `proc_macro`
│   ├── model.rs                # domain value types
│   ├── args/
│   │   ├── mod.rs              # re-exports; module documentation
│   │   ├── grammar/
│   │   │   ├── mod.rs          # pure key-shape policy (the proof target)
│   │   │   └── tests.rs        # exhaustive + property tests
│   │   └── parse.rs            # syn adapter: tokens -> keyed payloads
│   └── expand/
│       ├── mod.rs              # renderer: DylintExpect -> TokenStream
│       └── tests.rs            # unit, snapshot, property, BDD tests
└── tests/
    ├── applies_to_items.rs     # compile-level usage across item kinds
    ├── features/
    │   └── dylint_expect.feature
    ├── ui.rs                   # trybuild harness
    └── ui/                     # ten compile-fail fixtures plus .stderr
```

The hexagonal boundary here is narrow and real, and it is worth stating
precisely rather than gesturing at layers. The infrastructure in a procedural
macro is `proc_macro::TokenStream`: it exists only inside a compiler invocation
and cannot be constructed in a unit test. Everything else — `syn`, `quote`,
`proc_macro2` — is pure compile-time data with no ambient effects, so it is
legitimately part of the domain's vocabulary rather than something to abstract
away. `src/lib.rs` is therefore the sole driving adapter, and every other module
is domain logic that a plain `#[test]` can call directly. Within the domain,
`args/grammar` is deliberately isolated further, expressed over a three-symbol
enum with no `syn` types at all, because that is what makes it provable.

### Signatures that must exist at the end of EP-M2

In `crates/whitaker_support_macros/src/args/grammar/mod.rs`:

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
/// most one `reason`, in any order.
pub(crate) fn validate_key_sequence(keys: &[ArgKey]) -> Result<(), ArgShapeError>;
```

In `crates/whitaker_support_macros/src/model.rs`:

```rust
/// Names the Dylint library whose `dylint_lib` cfg gates the expectation.
pub(crate) struct LibraryName(String);

/// Carries the human-readable justification for a suppression.
pub(crate) struct Reason(String);

/// Holds a validated `dylint_expect` invocation.
pub(crate) struct DylintExpect {
    lib: LibraryName,
    lints: Vec<syn::Path>,
    reason: Option<Reason>,
}
```

In `crates/whitaker_support_macros/src/expand/mod.rs`:

```rust
/// Renders the attribute prelude and the untouched item.
pub(crate) fn expand(spec: &DylintExpect, item: &proc_macro2::TokenStream) -> proc_macro2::TokenStream;
```

In `crates/whitaker_support_macros/src/lib.rs`:

```rust
#[proc_macro_attribute]
pub fn dylint_expect(attr: TokenStream, item: TokenStream) -> TokenStream;
```

### Required expansion

For `lib = "whitaker_lints"`, `lints(no_std_fs_operations, module_max_lines)`,
`reason = "legacy call-site"`, the expansion is:

```rust,no_run
#[allow(clippy::allow_attributes, reason = "dylint_expect emits allow attributes by design")]
#[allow(unknown_lints, reason = "the gated lints are unknown outwith a Dylint run")]
#[allow(unexpected_cfgs, reason = "dylint_lib is injected by Dylint, not declared in check-cfg")]
#[cfg_attr(
    dylint_lib = "whitaker_lints",
    expect(no_std_fs_operations, module_max_lines, reason = "legacy call-site")
)]
fn read_legacy_config() {}
```

When `reason` is omitted, the `reason = "..."` key is omitted from `expect(...)`
only; the three `allow` attributes keep their fixed macro-authored reasons.
Those reasons justify the macro's own plumbing and are deliberately distinct
from the user's justification for the suppression.

### Dependencies

`crates/whitaker_support_macros/Cargo.toml`:

```toml
[package]
name = "whitaker_support_macros"
version = "0.2.7"
edition = "2024"
description = "Attribute macro for conditional Dylint expect suppressions"
license.workspace = true
repository.workspace = true
homepage.workspace = true
documentation.workspace = true

[lib]
proc-macro = true

[dependencies]
proc-macro2 = { workspace = true }
quote = { workspace = true }
syn = { workspace = true }

[dev-dependencies]
googletest = { workspace = true }
insta = { workspace = true }
pretty_assertions = { workspace = true }
proptest = { workspace = true }
rstest = { workspace = true }
rstest-bdd = { workspace = true }
rstest-bdd-macros = { workspace = true }
trybuild = { workspace = true }

[lints]
workspace = true
```

New `[workspace.dependencies]` entries in the root `Cargo.toml`, all with caret
requirements as `AGENTS.md` mandates:

```toml
proc-macro2 = "1.0.106"
quote = "1.0.46"
syn = { version = "2.0.119", features = ["full", "parsing", "printing", "proc-macro"] }
googletest = "0.14.2"
pretty_assertions = "1.4.1"
whitaker_support_macros = { path = "crates/whitaker_support_macros", version = "0.2.7" }
```

`googletest` and `pretty_assertions` are new to this workspace. The task
authorization explicitly names both as approved dependencies. Verify the exact
current versions with `cargo search` before pinning; the values above are
placeholders to be confirmed at EP-M1.

Existing `crates/whitaker_test_macros` declares `syn`, `quote`, and
`proc-macro2` directly rather than through the workspace. Promoting them to
`[workspace.dependencies]` and leaving `whitaker_test_macros` unchanged would
leave two pins for the same crates. Migrate `whitaker_test_macros` to the
workspace entries in the same commit — it is a three-line change with no
behavioural effect, and leaving the drift would be the worse outcome.

## Concrete steps

Run everything from the repository root,
`/home/leynos/.lody/repos/github---leynos---whitaker/worktrees/c0b1e9dd-2aff-4206-8bb8-19335d3fa354`.

Every gate command is piped through `tee` to a log under `/tmp`, because gate
output is long and the terminal truncates the middle:

```bash
set -o pipefail
make check-fmt 2>&1 | tee "/tmp/check-fmt-whitaker-$(git branch --show-current | tr '/' '-').out"
```

### Stage A commands

```bash
SCRATCH=$(mktemp -d /tmp/dylint-expect-probe.XXXXXX)
# 1. expect-viability probe: build a lint library, then compile a fixture that
#    does and does not trigger it under `#[expect(...)]`.
# 2. clippy self-suppression probe.
cargo clippy -- -D clippy::allow_attributes -D clippy::allow_attributes_without_reason
# 3. unmitigated-warning transcript (the INV-WARN-1 negative control).
# 4. kani probe.
cargo kani --help >/dev/null && echo "kani present"
rm -rf "$SCRATCH"
```

Record each transcript in `Artefacts and notes` before deleting the scratch
directory.

### Stage B and C commands

```bash
cargo nextest run -p whitaker_support_macros 2>&1 | tee /tmp/nextest-support-macros.out
cargo nextest run -p whitaker_support_macros -E 'test(exhaustive_key_sequences)'
TRYBUILD=overwrite cargo nextest run -p whitaker_support_macros -E 'binary(ui)'
INSTA_UPDATE=always cargo nextest run -p whitaker_support_macros
cargo insta review
make verus 2>&1 | tee /tmp/verus-support-macros.out
```

Expected Verus transcript once EP-M2 closes:

```plaintext
verification results:: 4 verified, 0 errors
```

`TRYBUILD=overwrite` and `INSTA_UPDATE=always` regenerate expected output.
Never commit output blessed this way without reading every regenerated file —
blessing blind converts a test into a tautology.

### Stage D commands

```bash
make check-fmt 2>&1 | tee /tmp/check-fmt-support-macros.out
make typecheck 2>&1 | tee /tmp/typecheck-support-macros.out
make lint      2>&1 | tee /tmp/lint-support-macros.out
make test      2>&1 | tee /tmp/test-support-macros.out
make markdownlint 2>&1 | tee /tmp/markdownlint-support-macros.out
make nixie        2>&1 | tee /tmp/nixie-support-macros.out
make publish-check PUBLISH_PACKAGES="whitaker_support_macros" 2>&1 \
  | tee /tmp/publish-check-support-macros.out
```

Delegate full gate runs to the `scrutineer` sub-agent rather than running them
in the planning context; it runs them sequentially, captures each log, and
returns a bounded report. Do not run gates in parallel — this environment uses
build caching, and sequential execution is what benefits from it.

## Validation and acceptance

A reader can confirm this work by doing the following.

Write a file `/tmp/check.rs` containing a function annotated with
`#[whitaker_support_macros::dylint_expect(lib = "whitaker_lints", lints(
no_std_fs_operations), reason = "probe")]` inside the crate's own test tree,
and run `make test`. Expect a clean pass with no warnings. Then run `make lint`
and expect the same. That is INV-WARN-1 in observable form.

Introduce a deliberate error — remove the `lints(...)` argument from one of the
`trybuild` fixtures' sibling files — and expect a compiler error reading
``dylint_expect` requires a `lints(...)` argument with at least one lint path``
anchored at the attribute's span.

### Red-Green-Refactor evidence to record

- Red: `cargo nextest run -p whitaker_support_macros` at the end of Stage B
  fails to compile, with errors naming `validate_key_sequence`, `expand`, and
  `DylintExpect` as unresolved. That is the intended failure reason — the tests
  specify an API that does not yet exist.
- Green: the same command at the end of Stage C reports all tests passed.
  Record the exact count.
- Refactor: after extracting any helper that keeps a file under 400 lines,
  re-run `cargo nextest run -p whitaker_support_macros` and then `make lint`,
  and expect both to pass unchanged.

### Verification evidence to record

- `make verus` reports `0 errors` and names the four discharged obligations.
- Each negative control from `Verification plan` has been run, produced the
  predicted failure, and been reverted. Record the failure output for each; a
  verification suite whose negative controls were never exercised is not
  evidence.
- `proptest` classification output for INV-EXP-3 shows at least 20% of
  generated lint lists contain a repeated path.

### Quality criteria

- Tests: `make test` passes; the new crate's tests all pass under `cargo
  nextest run -p whitaker_support_macros`.
- Verification: INV-SHAPE-1 and INV-SHAPE-2 discharged in Verus with zero
  errors and zero remaining `assume` statements; INV-EXP-1 through INV-EXP-3
  and INV-DIAG-1 discharged by their named artefacts; INV-WARN-1 discharged by
  `make lint` contrasted against the EP-M0 negative control.
- Lint and typecheck: `make check-fmt`, `make typecheck`, and `make lint` all
  exit 0.
- Documentation: `make markdownlint` and `make nixie` exit 0.
- Packaging: `make publish-check PUBLISH_PACKAGES="whitaker_support_macros"`
  exits 0.
- Performance: no threshold. The macro runs at compile time on a handful of
  tokens; ADR 002 §Known risks already accepts the modest `syn`/`quote`
  compile-time cost.
- Security: none beyond the workspace's existing `unsafe_code = "forbid"`.

## Idempotence and recovery

Every step is re-runnable. The crate directory can be deleted and recreated
without touching any other workspace member, because `crates/*` globbing means
there is no `members` list to keep in step.

Snapshot and `trybuild` fixtures are regenerable, but regeneration is not
recovery — a regenerated expectation must be read before it is committed.

The Verus sidecar is independent of the Cargo build; a broken proof file fails
`make verus` and nothing else.

Nothing in this plan publishes anything. `cargo publish` runs only from
`release.yml` on a release tag, so the release wiring is inert until a tag is
pushed. Reverting the two workflow edits fully undoes EP-M4.

The Stage A scratch directory lives under `/tmp` and must be deleted at the end
of EP-M0. `/tmp` has 32 GB; if it fills, stop and report rather than working
around it.

## Artefacts and notes

### Feature specification

`crates/whitaker_support_macros/tests/features/dylint_expect.feature`:

```gherkin
Feature: Conditional Dylint expect suppression

  Scenario: A single lint is gated behind the named library
    Given a dylint_expect invocation naming library "whitaker_lints"
    And the invocation lists the lint "no_std_fs_operations"
    When the attribute is expanded
    Then the expansion gates an expect attribute on cfg dylint_lib "whitaker_lints"
    And the expect attribute names "no_std_fs_operations"

  Scenario: Several lints keep their source order
    Given a dylint_expect invocation naming library "whitaker_lints"
    And the invocation lists the lints "module_max_lines, no_std_fs_operations"
    When the attribute is expanded
    Then the expect attribute names the lints in the order "module_max_lines, no_std_fs_operations"

  Scenario: A supplied reason reaches the expect attribute
    Given a dylint_expect invocation naming library "whitaker_lints"
    And the invocation lists the lint "no_std_fs_operations"
    And the invocation supplies the reason "legacy call-site"
    When the attribute is expanded
    Then the expect attribute carries the reason "legacy call-site"

  Scenario: An omitted reason leaves the expect attribute without one
    Given a dylint_expect invocation naming library "whitaker_lints"
    And the invocation lists the lint "no_std_fs_operations"
    When the attribute is expanded
    Then the expect attribute carries no reason

  Scenario: Arguments may be supplied in any order
    Given a dylint_expect invocation whose arguments are ordered "reason, lints, lib"
    When the attribute is expanded
    Then the expansion matches the canonical ordering

  Scenario: A duplicated argument is rejected
    Given a dylint_expect invocation that supplies "lib" twice
    When the attribute is expanded
    Then expansion fails reporting a duplicate "lib" argument

  Scenario: An empty lint list is rejected
    Given a dylint_expect invocation naming library "whitaker_lints"
    And the invocation lists no lints
    When the attribute is expanded
    Then expansion fails reporting that at least one lint path is required
```

Wire each scenario with `#[scenario(path = "tests/features/dylint_expect.feature",
index = N)]` from an in-crate `#[cfg(test)]` module, following the pattern in
`crates/bumpy_road_function/tests/analysis_behaviour.rs`. The `path` is
resolved relative to `CARGO_MANIFEST_DIR`, so an in-crate unit-test module can
reach a feature file under `tests/`.

### Compile-fail fixture inventory

Ten fixtures under `crates/whitaker_support_macros/tests/ui/`, each paired with
a reviewed `.stderr`: `missing_lib.rs`, `missing_lints.rs`, `empty_lints.rs`,
`duplicate_lib.rs`, `duplicate_lints.rs`, `duplicate_reason.rs`,
`non_string_lib.rs`, `non_string_reason.rs`, `unknown_argument.rs`, and
`empty_argument_list.rs`. All ten `.stderr` files must differ from one another;
two identical files mean two error classes were collapsed.

### EP-M0 transcripts

To be recorded here during Stage A. Leave this subsection in place with the
heading and fill it in; do not delete it if the probes are inconclusive —
record the inconclusive result and escalate.

## Progress

- [ ] EP-M0: Stage A prototyping — the four empirical questions answered and
      transcripts recorded.
- [ ] EP-M1: crate skeleton, manifest, and workspace dependency entries.
- [ ] EP-M2: grammar, parser, expansion, all tests green, Verus proof
      discharged, snapshots and `.stderr` fixtures reviewed and blessed.
- [ ] EP-M3: warning-free under the full workspace lint policy; Makefile
      `WHITAKER_PACKAGES` updated.
- [ ] EP-M4: publish metadata verified; `ci.yml` and `release.yml` wired.
- [ ] EP-M5: ADR 002 accepted; suite design, repository layout, developers'
      guide, and users' guide updated; roadmap 1.3.1 marked done.

## Surprises & discoveries

None yet. Record Stage A findings here as they arrive, with evidence and
impact, before acting on them.

## Decision log

- Decision (D-1): scope 1.3.1 to the macro crate alone, and use the
  `#[whitaker_support_macros::dylint_expect(...)]` path throughout its own
  tests and documentation.
  Rationale: ADR 002 §Migration plan phase 1 covers exactly this crate, and
  roadmap 1.3.2 owns the `whitaker_support` facade. Introducing a placeholder
  facade now would be compatibility theatre — there is no consumer to be
  compatible with, and 1.3.2 adds the real re-export without changing anything
  delivered here.
  Date/Author: 2026-08-21, planning agent.

- Decision (D-2): make the crate publish-ready and wire the release pipeline
  within 1.3.1.
  Rationale: user direction at plan approval, resolving ADR 002 §Outstanding
  decisions item 3. ADR 002's comparison table claims Option C "works in
  downstream crates without extra config", which is only true if the crate is
  published. Nothing is actually published until a release tag is pushed, so
  the wiring is inert and reversible.
  Date/Author: 2026-08-21, user.

- Decision (D-3): move ADR 002 from `Proposed` to `Accepted` within 1.3.1.
  Rationale: user direction. Implementing the decision commits to it. The
  status change carries a dated summary and must incorporate the EP-M0
  findings, so acceptance is evidence-backed rather than nominal.
  Date/Author: 2026-08-21, user.

- Decision (D-4): add `reason = "..."` to each of the three `allow` attributes
  the expansion emits, beyond ADR 002's literal text.
  Rationale: this workspace denies `clippy::allow_attributes_without_reason`.
  ADR 002's expansion as written would fail Whitaker's own lint policy, which
  cannot be the intent of a document whose stated goal is warning-free output
  (ADR002-TR-1). The addition is mechanical and changes no requirement, so it
  is recorded here rather than raised as an architecture deviation. It must be
  reflected in ADR 002 §Decision outcome at EP-M5.
  Date/Author: 2026-08-21, planning agent.

- Decision (D-5): keep the macro-authored `allow` reasons distinct from the
  user's `reason` argument.
  Rationale: they justify different things. The `allow` reasons explain the
  Dylint plumbing and are identical at every call-site; the user's reason
  explains why this particular suppression exists. Merging them would make
  every expansion claim the user's justification for the plumbing, which is
  misleading at review time.
  Date/Author: 2026-08-21, planning agent.

- Decision (D-6): use Verus plus exhaustive enumeration rather than Kani for
  the argument-grammar obligations.
  Rationale: Kani cannot compile a `proc-macro = true` crate, and relocating
  the grammar policy out of the crate purely to satisfy a tool would let tool
  choice dictate architecture. The substitute is stronger for this obligation,
  not weaker: with a three-symbol alphabet, exhaustive enumeration to length 4
  is a complete decision procedure within that bound — everything a bounded
  model check would deliver — while the Verus proof covers the unbounded case
  that Kani could never reach. EP-M0 confirms the Kani limitation empirically
  rather than assuming it.
  Date/Author: 2026-08-21, planning agent.

- Decision (D-7): do not build a three-configuration compatibility harness in
  1.3.1.
  Rationale: roadmap 1.3.3 is explicitly "Add compatibility coverage that
  proves the attribute stays warning free in non-Dylint builds, Clippy runs,
  and Dylint runs with the matching `dylint_lib` value". Building it here would
  duplicate that task. 1.3.1 covers the non-Dylint configuration only, and gets
  it free: the crate's own tests and doctests use the macro and are compiled
  under `-D warnings` with the full workspace lint policy.
  Date/Author: 2026-08-21, planning agent.

- Decision (D-8): migrate `crates/whitaker_test_macros` to the newly promoted
  workspace pins for `syn`, `quote`, and `proc-macro2`.
  Rationale: promoting those crates to `[workspace.dependencies]` for the new
  crate while leaving the existing one on its own pins would create two
  versions of truth for the same dependency. The migration is three lines and
  behaviourally inert.
  Date/Author: 2026-08-21, planning agent.

## Outcomes & retrospective

To be completed at EP-M5. Before setting this plan to `COMPLETE`, reconcile
every EP-M0 discovery against ADR 002:

- If the `expect`-viability probe contradicts ADR 002 §Decision outcome, the
  ADR must be amended and re-accepted, not worked around.
- If the `unknown_lints` masking hazard is confirmed, it must appear in ADR 002
  §Known risks and in the users' guide.
- Purely mechanical differences from ADR 002's text — the D-4 reason additions
  are the known example — are recorded in `Decision log` and reflected in the
  ADR, and require no requirements change.

Do not mark this plan `COMPLETE` while any upstream change or deviation remains
unrecorded.

## Signposts

Documentation to read before starting:

- `docs/adr-002-dylint-expect-attribute-macro.md` — the governing decision.
- `docs/whitaker-dylint-suite-design.md` — how the suite is assembled and where
  support crates sit.
- `docs/repository-layout.md` — the directory map, and where the new crate is
  listed.
- `docs/documentation-style-guide.md` — ADR section requirements, sentence-case
  headings, 80-column prose wrapping, table and figure captions, en-GB-oxendict
  spelling.
- `docs/developers-guide.md` §Creating a New Lint — the neighbourhood where the
  new convention section belongs.
- `docs/rust-testing-with-rstest-fixtures.md` — fixture and parameterization
  conventions.
- `docs/rstest-bdd-users-guide.md` — `#[scenario]` wiring and step-definition
  layout for the v0.5 API.
- `docs/rust-doctest-dry-guide.md` — keeping the rustdoc examples that
  `AGENTS.md` requires from duplicating test logic.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` — the standard
  the 400-line and small-function rules serve.
- `AGENTS.md` — the binding style, testing, dependency, and commit rules.

Skills to load before starting:

- `leta` — semantic navigation. Load at session start and prefer it to text
  search for any symbol lookup.
- `rust-router` — routes to the narrower Rust skills; load it first and follow
  where it points.
- `arch-crate-design` — crate boundaries, `publish` decisions, and public
  versus internal API shape, all of which this plan touches.
- `rust-unit-testing` — `rstest` parameterization, `googletest` matchers,
  `pretty_assertions`, and `insta` snapshot discipline.
- `proptest` — strategy design and shrinking for INV-EXP-3 and INV-SHAPE-1.
- `verus` — the deductive proof for INV-SHAPE-1 and INV-SHAPE-2, especially the
  trigger and `assert(...) by { ... }` guidance.
- `hexagonal-architecture` — the port boundary rationale in
  `Interfaces and dependencies`.
- `arch-decision-records` — for the ADR 002 status change at EP-M5.
- `execplans` — for keeping this document current as work proceeds.
- `commit-message` — file-based commit messages, never `-m`.
- `nextest` — filtersets for the focused test commands above.

Sub-agents to use:

- `scrutineer` — the exclusive runner of full commit gates. Delegate every
  `make` gate run to it; read its cited log rather than re-running a gate.
- `scribe` — the documentation edits at EP-M5.
- `wyvern` — read-only reconnaissance when a file's shape is unclear.
- `alchemist` — only for a single falsifiable hypothesis with a supplied
  prediction and minimal experiment, such as one of the Stage A probes.

## Revision note

Initial draft, 2026-08-21. Covers roadmap 1.3.1 in full, with scope extended by
user direction at approval time to include crates.io publish wiring (D-2), the
ADR 002 status change (D-3), and the Dylint suite design cross-reference. No
work has begun; the plan awaits approval.
