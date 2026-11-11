# Architectural decision record (ADR) 002: Attribute macro for conditional Dylint `expect`

## Status

Accepted, 2026-08-21.

Whitaker adopts Option C, a procedural attribute macro, but with a materially
smaller expansion than this record originally proposed. Empirical testing
during the roadmap 1.3.1 planning phase established that the originally
specified four-attribute expansion does not achieve its own stated goal, and
that two of its attributes address diagnostics that never fire while a third
suppresses the only signal that catches a misspelt lint name. The expansion is
now the `cfg_attr` gate alone, paired with a one-line `check-cfg` entry in the
consuming manifest. See §Decision outcome / proposed direction and
§Known risks and limitations for the evidence.

## Date

2026-02-23. Amended and accepted 2026-08-21.

## Context and problem statement

Whitaker enforces project-specific Rust conventions using Dylint lint
libraries. These lints run outwith normal `cargo check` and `cargo test`
workflows.

Occasionally, a lint must be suppressed for a narrow scope (for example, a
legacy call-site that cannot be refactored immediately, or an intentional
exception that documents a policy boundary). Rust’s `#[expect(...)]` attribute
supports “temporary suppression” by emitting a diagnostic if the lint stops
triggering, which encourages removal of the suppression when it becomes
obsolete.

Direct use of `#[allow(<dylint_lint>)]` or `#[expect(<dylint_lint>)]` causes
noise in normal builds because:

- `rustc` does not know Dylint-defined lint names during ordinary compilation,
  so it can emit `unknown_lints` diagnostics.
- The recommended Dylint gating mechanism uses `cfg_attr(dylint_lib = "…", …)`,
  but toolchains can emit `unexpected_cfgs` diagnostics for unknown cfg
  keys/values when `check-cfg` validation is enabled.

The project needs an ergonomic, consistent, and low-friction mechanism for
annotating items with conditional Dylint `expect` semantics that:

- avoids polluting normal builds with warnings,
- keeps suppressions narrowly scoped to the item they justify, and
- stays legible during code review.

## Decision drivers

- Keep suppression annotations close to the affected item to support review and
  later refactoring.
- Minimize boilerplate and avoid copy-paste divergence across the codebase.
- Avoid requiring workspace-wide configuration changes for every downstream
  crate consuming Whitaker. _Not achieved; see the amendment to §Options
  considered, Option D. One `check-cfg` manifest entry is unavoidable._
- Prefer `expect` over `allow` where appropriate, to detect stale suppressions.
- Preserve compatibility with Clippy configurations that lint `#[allow]` usage.

## Requirements

### Functional requirements

- Provide an item attribute usable on functions, impl blocks, modules, and
  other Rust items.
- Support one or multiple lint names per annotation.
- Support an optional human-readable reason.
- Enable `#[expect(...)]` only when Dylint runs the specified lint library.

### Technical requirements

- Avoid warnings in non-Dylint builds, including:
  - `unknown_lints` for Dylint lint names,
  - `unexpected_cfgs` for `dylint_lib`, and
  - `clippy::allow_attributes` where enabled.

  > **Amendment, 2026-08-21.** Of these three, only `unexpected_cfgs` actually
  > occurs for the gated form, and the macro cannot suppress it — a manifest
  > `check-cfg` entry is required. The other two arise only if the expansion
  > itself emits `allow` attributes, which it no longer does. This requirement
  > is met by the macro plus that one manifest line, not by the macro alone.
- Keep the macro’s expansion explicit and reviewable.
- Maintain a clear separation between proc-macro code and lint implementation
  code.
- Document limitations for “pre-expansion” lints where conditional gating can
  misbehave (for example, a `#[derive(...)]` macro can emit diagnostics before
  `cfg_attr` is applied).

## Options considered

### Option A: Document a manual annotation pattern

Document and enforce a convention such as:

```rust,no_run
#[allow(unknown_lints)]
#[allow(unexpected_cfgs)]
#[cfg_attr(dylint_lib = "whitaker_suite", expect(no_std_fs_operations))]
fn f() {}
```

This option avoids new crates and dependencies, but it increases boilerplate
and encourages inconsistencies.

### Option B: Use a `macro_rules!` wrapper

Define `dylint_expect!("lib", lint, item)` and wrap items.

This option reduces boilerplate, but it does not provide a true attribute.
Call-sites become visually noisy and can feel alien in idiomatic Rust code.

### Option C: Provide a procedural attribute macro

Add a proc-macro attribute usable as:

```rust,no_run
#[whitaker_support::dylint_expect(
    lib = "whitaker_suite",
    lints(no_std_fs_operations),
    reason = "legacy exception; remove after refactor"
)]
fn f() {}
```

The macro expands to a `cfg_attr(...)` gate, enabling `expect(...)` only when
Dylint runs the relevant library. It originally also emitted three `allow(...)`
attributes; see §Decision outcome / proposed direction for why they were
removed.

### Option D: Rely on workspace `check-cfg` allowlists

Add `cfg(dylint_lib, values(any()))` to a workspace `check-cfg` allowlist and
write the gated attribute by hand.

> **Amendment, 2026-08-21.** This option was originally rejected on the grounds
> that it "keeps only `allow(unknown_lints)`" and "does not address Clippy's
> `allow_attributes` lint". **Both premises were wrong**, and the correction
> matters because it changes what Option C must do.
>
> Measured against Whitaker's own lint policy, a bare
> `#[cfg_attr(dylint_lib = "…", expect(…))]` with no `allow` attributes emits
> exactly one diagnostic: `unexpected_cfgs`. It does not emit `unknown_lints`,
> because a false `cfg_attr` predicate is stripped before lint-attribute
> processing, so `rustc` never sees the gated lint name. It does not emit
> `clippy::allow_attributes`, because there are no `allow` attributes present
> to lint. Adding the `check-cfg` entry reduces the diagnostic count to zero.
>
> Option D is therefore both necessary and sufficient for warning-free
> compilation. It remains true that it requires a manifest line in each
> consuming workspace and does nothing about boilerplate, which is why Option C
> is still adopted — but Option C is adopted for **ergonomics and a validation
> point**, not because it can avoid the `check-cfg` entry. It cannot: see
> §Known risks and limitations.

| Topic                                           | Option A | Option B | Option C | Option D |
| ----------------------------------------------- | -------- | -------- | -------- | -------- |
| Attribute ergonomics                            | Medium   | Low      | High     | Medium   |
| Boilerplate at call-site                        | High     | Low      | Low      | Medium   |
| Dependency footprint                            | Low      | Low      | Medium   | Low      |
| Review clarity                                  | Medium   | Medium   | High     | Medium   |
| Works in downstream crates without extra config | Low      | Low      | Low      | Low      |
| Risk of masking cfg issues on an item           | Medium   | Medium   | Low      | Low      |

_Table 1: Trade-offs between approaches for conditional Dylint suppression._

> **Amendment, 2026-08-21.** Two rows were corrected. "Works in downstream
> crates without extra config" was scored High for Options A, B and C and Low
> for Option D; every option is in fact Low, because none of them can suppress
> `unexpected_cfgs` without the consuming manifest carrying a `check-cfg` entry.
> That row was the main reason Option C was preferred over Option D, so its
> correction removes the original decisive argument — Option C is retained on
> ergonomics, review clarity, and having one place to add validation. "Risk of
> masking cfg issues on an item" drops to Low for Option C now that the
> expansion emits no `allow(unexpected_cfgs)`.

## Decision outcome / proposed direction

Adopt Option C, with the expansion amended as set out below.

Whitaker will add a small support layer that provides an attribute macro
`dylint_expect` following the procedural approach:

- Create `whitaker_support_macros` as a `proc-macro = true` crate.
- Create `whitaker_support` as a normal crate that re-exports the macro.
- Implement `#[whitaker_support::dylint_expect(...)]` with arguments:
  - `lib = "..."` (string literal),
  - `lints(path, ...)` (one or more lint paths), and
  - optional `reason = "..."`.
- Expand the attribute to the cfg-gated expectation and nothing else:

  ```rust,no_run
  #[cfg_attr(
      dylint_lib = "whitaker_suite",
      expect(no_std_fs_operations, reason = "legacy call-site")
  )]
  fn read_legacy_config() {}
  ```

- Require consuming workspaces to carry one manifest entry, which is what
  actually makes the gated form warning-free:

  ```toml
  [workspace.lints.rust]
  unexpected_cfgs = { level = "warn", check-cfg = ['cfg(dylint_lib, values(any()))'] }
  ```

### Amendment, 2026-08-21: why the expansion shrank

This record originally specified an expansion carrying
`#[allow(clippy::allow_attributes)]`, `#[allow(unknown_lints)]`, and
`#[allow(unexpected_cfgs)]` alongside the gate. Testing during roadmap 1.3.1
planning, including a spike that built a real proc-macro crate emitting exactly
those four attributes, established three things:

1. **The expansion did not achieve its purpose.** A _sibling_
   `#[allow(unexpected_cfgs)]` does not suppress `unexpected_cfgs` arising from
   a `cfg_attr` on the same item, because that diagnostic is resolved during
   cfg-expansion, before the annotated item's own lint levels are in scope. The
   suppression works only from an enclosing module, a crate-level inner
   attribute, or the manifest. An attribute macro cannot reach any of those for
   an arbitrary item without wrapping it and changing its semantics.
2. **Two of the three `allow` attributes were inert.** Neither `unknown_lints`
   nor `clippy::allow_attributes` fires on the gated form; see §Options
   considered, Option D.
3. **`#[allow(unknown_lints)]` was actively harmful.** Inside a Dylint run, a
   misspelt lint name is caught by `unknown_lints`. Suppressing it converts
   every typo into a suppression that compiles cleanly and silences nothing.

Omitting the `allow` attributes therefore makes the macro both simpler and
safer. The `check-cfg` entry is not an optional extra; it is the mechanism.

Whitaker will document the intended usage and limitations, including known
misbehaviour for pre-expansion lints where `cfg_attr` gating may not apply in
time.

## Goals and non-goals

- Goals:
  - Reduce boilerplate for conditional Dylint suppressions.
  - Standardize suppression semantics across Whitaker and downstream crates.
  - Encourage removal of stale suppressions via `expect`.
- Non-goals:
  - Guarantee correct behaviour for pre-expansion lints.
  - Replace workspace-level `check-cfg` allowlists for teams that prefer them.
  - Provide a general-purpose lint suppression framework beyond Whitaker’s
    Dylint integration.

## Migration plan

### Phase 1: Introduce support crates

- Add `crates/whitaker_support_macros` with the proc-macro implementation.
- Add `crates/whitaker_support` to re-export the attribute.
- Add API documentation and examples.

### Phase 2: Add correctness and compatibility tests

- Add a small compile-test fixture crate that:
  - builds without Dylint configured,
  - runs under Clippy,
  - runs under Dylint with `dylint_lib = "whitaker_suite"` set.
- Validate that the macro emits no warnings under expected configurations.

### Phase 3: Adopt the attribute in Whitaker-managed code

- Replace ad-hoc `allow/expect` sequences with
  `#[whitaker_support::dylint_expect]`.
- Add guidance for reviewers: prefer `expect` for temporary suppressions.

## Known risks and limitations

- **The aggregated suite does not honour lint-level attributes.** This is the
  most serious limitation, and it blocks the macro from being useful in its
  target configuration. A controlled experiment — identical fixture, identical
  source revision, identical toolchain, only the loaded library differing —
  showed that `libno_std_fs_operations` honours `#[allow]` and `#[expect]`
  correctly, while `libwhitaker_suite` ignores both and additionally emits a
  spurious `unfulfilled_lint_expectations` warning for every `expect`. Since
  `whitaker_suite` is what `whitaker --all`, `make lint-whitaker`, and every
  installed consumer load, no attribute-based suppression currently works in
  practice. This must be fixed before ADR migration phase 3. It is tracked
  separately from the 1.3.x macro work, which does not depend on it for
  delivery.
- **`docs/users-guide.md` currently overstates attribute support.** It states
  that `#[allow(no_std_fs_operations)]` "works … since the lint honours Rust's
  lint-level attributes". That is true of an individual lint library and false
  of the aggregated suite. Correct it alongside the fix above.
- **One `lib` value cannot cover both deployment modes.** Whitaker ships both
  an aggregated `whitaker_suite` cdylib and per-lint libraries. Which
  `dylint_lib` cfg is set depends on what the consumer installed, so a
  suppression naming one is inert for a consumer who loaded the other. Stacking
  two attributes is the interim workaround. See §Outstanding decisions for the
  reserved extension.
- **A misspelt lint name or a hyphenated `lib` produces a silent no-op.** The
  amended expansion preserves `unknown_lints` as the safety net for the first,
  once the suite bug above is fixed. The macro rejects an empty or hyphenated
  `lib` at expansion time. A wrong-but-well-formed `lib` value cannot be caught
  by the macro at all; only a lint with access to the loaded library set can.
- Proc-macro dependencies (`syn`, `quote`) increase compile-time for crates that
  depend on the macro. The impact should remain modest given the small surface
  area, and `syn` is pinned without its `full` feature.
- Pre-expansion lints can bypass `cfg_attr` gating. For example, a
  `#[derive(...)]` macro can raise lint diagnostics on generated code before
  `dylint_expect` expansion is applied. The macro cannot correct toolchain
  ordering constraints.
- The `check-cfg` entry is a prerequisite, not a convenience. Without it every
  annotated item emits `unexpected_cfgs`, and the macro cannot suppress that on
  the consumer's behalf.

## Outstanding decisions

Resolved on 2026-08-21:

- **Publishing.** Both `whitaker_support_macros` and `whitaker_support` are
  published to crates.io. Both names were confirmed available. The macro crate
  publishes last in the release sequence so a failure cannot strand
  `whitaker-common` or `whitaker-installer`.
- **`check-cfg` allowlists.** No longer a "secondary mitigation" — the
  `cfg(dylint_lib, values(any()))` entry is the primary and only mechanism that
  suppresses `unexpected_cfgs`, and is a documented prerequisite. See §Options
  considered, Option D.

Still open:

- Whether to also provide `dylint_allow` for cases where `expect` is not
  appropriate. If added, it shares the same argument grammar, so the grammar
  should live where both can reach it.
- **Reserved extension for multiple libraries.** To cover both the aggregated
  and per-lint deployment modes in one annotation, the `lib` key may later
  accept either `lib = "x"` or `lib("a", "b")`. Reserving the payload shape on
  the existing key, rather than adding a fourth `libs` key, keeps the
  argument-key grammar and its validation unchanged. Decide before the argument
  surface is frozen by adoption.
- Confirm the final ADR sequence number for the Whitaker repository.

## Architectural rationale

Whitaker aims to enforce policy through tooling while keeping the codebase
pleasant to maintain. A small, explicit support layer localizes Dylint-specific
compilation quirks behind a stable, reviewable interface.

The attribute macro approach keeps suppressions close to the code they justify,
reduces drift across crates, and supports the intended “temporary suppression”
semantics of `expect` without requiring downstream projects to adopt global
configuration changes.
