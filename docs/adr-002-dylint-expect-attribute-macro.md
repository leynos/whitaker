# Architectural decision record (ADR) 002: Attribute macro for conditional Dylint `expect`

## Status

Proposed.

## Date

2026-02-23.

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
  crate consuming Whitaker.
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
#[cfg_attr(dylint_lib = "whitaker_lints", expect(whitaker::some_lint))]
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
    lib = "whitaker_lints",
    lints(whitaker::some_lint),
    reason = "legacy exception; remove after refactor"
)]
fn f() {}
```

The macro expands to a standard set of `allow(...)` and `cfg_attr(...)`
attributes, enabling `expect(...)` only when Dylint runs the relevant library.

### Option D: Rely on workspace `check-cfg` allowlists

Add `cfg(dylint_lib, values(any()))` to a workspace `check-cfg` allowlist,
reduce `unexpected_cfgs` warnings, and keep only `allow(unknown_lints)`.

This option improves signal-to-noise, but it requires configuration changes in
consuming workspaces and does not address boilerplate or Clippy’s
`allow_attributes` lint.

| Topic                                           | Option A | Option B | Option C | Option D |
| ----------------------------------------------- | -------- | -------- | -------- | -------- |
| Attribute ergonomics                            | Medium   | Low      | High     | Medium   |
| Boilerplate at call-site                        | High     | Low      | Low      | Medium   |
| Dependency footprint                            | Low      | Low      | Medium   | Low      |
| Review clarity                                  | Medium   | Medium   | High     | Medium   |
| Works in downstream crates without extra config | High     | High     | High     | Low      |
| Risk of masking cfg issues on an item           | Medium   | Medium   | Medium   | Low      |

_Table 1: Trade-offs between approaches for conditional Dylint suppression._

## Decision outcome / proposed direction

Adopt Option C.

Whitaker will add a small support layer that provides an attribute macro
`dylint_expect` following the procedural approach:

- Create `whitaker_support_macros` as a `proc-macro = true` crate.
- Create `whitaker_support` as a normal crate that re-exports the macro.
- Implement `#[whitaker_support::dylint_expect(...)]` with arguments:
  - `lib = "..."` (string literal),
  - `lints(path, ...)` (one or more lint paths), and
  - optional `reason = "..."`.
- Expand the attribute to include the following:
  - `#[allow(clippy::allow_attributes)]`,
  - `#[allow(unknown_lints)]`,
  - `#[allow(unexpected_cfgs)]`, and
  - `#[cfg_attr(dylint_lib = "...", expect(...))]`.

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
  - runs under Dylint with `dylint_lib = "whitaker_lints"` set.
- Validate that the macro emits no warnings under expected configurations.

### Phase 3: Adopt the attribute in Whitaker-managed code

- Replace ad-hoc `allow/expect` sequences with
  `#[whitaker_support::dylint_expect]`.
- Add guidance for reviewers: prefer `expect` for temporary suppressions.

## Known risks and limitations

- `#[allow(unexpected_cfgs)]` can mask unrelated cfg mistakes inside the
  annotated item. Reviewers should keep annotations narrowly scoped.
- Proc-macro dependencies (`syn`, `quote`) increase compile-time for crates that
  depend on the macro. The impact should remain modest given the small surface
  area.
- Pre-expansion lints can bypass `cfg_attr` gating. For example, a
  `#[derive(...)]` macro can raise lint diagnostics on generated code before
  `dylint_expect` expansion is applied. The macro cannot correct toolchain
  ordering constraints.
- The `lib` value must match the identifier Dylint injects via `dylint_lib`.
  Mismatches silently disable the `expect` and can lead to missed enforcement.

## Outstanding decisions

- Confirm the final ADR sequence number for the Whitaker repository.
- Decide whether to also provide `dylint_allow` for cases where `expect` is not
  appropriate.
- Decide whether to publish `whitaker_support` to crates.io or keep it as a
  workspace-only utility.
- Decide whether to recommend workspace `check-cfg` allowlists as a secondary,
  non-macro mitigation.

## Architectural rationale

Whitaker aims to enforce policy through tooling while keeping the codebase
pleasant to maintain. A small, explicit support layer localizes Dylint-specific
compilation quirks behind a stable, reviewable interface.

The attribute macro approach keeps suppressions close to the code they justify,
reduces drift across crates, and supports the intended “temporary suppression”
semantics of `expect` without requiring downstream projects to adopt global
configuration changes.
