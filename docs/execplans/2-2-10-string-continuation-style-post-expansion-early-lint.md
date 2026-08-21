# Implement `string_continuation_style` as a post-expansion early lint

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`,
`Decision log`, `Outcomes & retrospective`, `Conformance basis`, and
`Verification plan` must be kept up to date as work proceeds.

Status: DRAFT

## Purpose / big picture

Whitaker's house style says a long string literal should be spelled with
`concat!()` rather than an escaped newline continuation. Reviewers keep
applying that rule by hand, and they keep getting it wrong: some
continuations cannot be replaced without changing what the program does.

After this change, a Rust author who writes

```rust
let text = "alpha \
            beta";
```

and runs `cargo whitaker` (or `cargo dylint --lib string_continuation_style`)
sees a warning offering a machine-applicable fix that rewrites the expression
to `concat!("alpha ", "beta")`, byte-for-byte equivalent at runtime. The same
author who writes

```rust
let header = format!(
    "HTTP/1.1 {status_line}\r\nContent-Length: {}\r\n\
     Content-Type: application/octet-stream\r\n\r\n",
    body.len(),
);
```

sees **nothing**, because `format!(concat!(...))` cannot implicitly capture
`status_line` and the rewrite would not compile. The lint is deliberately
asymmetric: it fires only when the replacement is proven safe.

Observable success:

1. `cargo test -p string_continuation_style` passes, including a UI matrix
   that pins both the firing and non-firing cases.
2. The suite library reports the new lint:
   `cargo dylint list` includes `string_continuation_style`.
3. `docs/users-guide.md` documents the rule and its exemptions.
4. Running the lint over Whitaker itself produces no findings on the
   pull-request-296 fixture reproduced in `crates/string_continuation_style/ui/`.

## Constraints

Hard invariants. Violating one requires escalation, not a workaround.

- **Evaluated-value preservation.** A suggested rewrite must produce a string
  whose evaluated bytes are identical to the original literal's. There is no
  acceptable margin here.
- **Near-zero false positives.** When the lint cannot prove safety it must be
  silent. It must never emit an unfixable style complaint. Prefer
  `Ignore`/`RequireContinuation` over a speculative diagnostic.
- **No macro-argument surgery.** The lint must not add explicit format
  arguments (`status_line = status_line`) to unlock a rewrite. See RFC 0002
  §Non-goals.
- **Module size.** No source file added by this work may exceed 400 lines
  (`AGENTS.md` §Code style, enforced by Whitaker's own `module_max_lines`).
- **British English.** Comments and documentation use en-GB-oxendict spelling
  (`AGENTS.md`), except where quoting an external API.
- **Domain purity.** The pure classification, scanning, and rewrite-construction
  logic must not import `rustc_*` types. It must be unit-testable without a
  compiler session. See §Interfaces and dependencies.
- **No changes to existing lints' behaviour.** This plan adds a lint; it must
  not alter the diagnostics of the nine existing standard lints.
- **Suite structure.** Do not force an early pass into
  `rustc_lint::late_lint_methods!`/`declare_combined_late_lint_pass!`. Register
  it separately (RFC 0002 §Suite integration).
- **Toolchain.** `rust-toolchain.toml` pins `nightly-2026-05-28`. Do not bump it.

## Tolerances (exception triggers)

- **Scope.** If implementation requires touching more than 45 files, or more
  than 3500 net lines outside `crates/string_continuation_style/`, stop and
  escalate.
- **Dependencies.** Three dependency additions are pre-authorized (see
  `DEC-006`): `googletest`, `pretty_assertions`, and promoting `serial_test`
  into `[workspace.dependencies]`. A fourth (`rustc-literal-escaper`, see
  `DEC-007`) is proposed but **requires approval before use**. Any dependency
  beyond these four: stop and escalate.
- **Public interface.** If `whitaker-common`'s public API must change in a way
  that is not purely additive, stop and escalate.
- **Architecture.** If milestone `EP-M0` concludes that a pure early pass
  cannot soundly prove macro identity for the `log`/`tracing` families, stop at
  the `EP-M0` go/no-go and present the options in `DEC-002` before continuing.
- **Iterations.** If a UI fixture's `.stderr` cannot be made stable after 4
  attempts, stop and escalate; unstable diagnostic output usually means the
  span recovery is wrong, not that the fixture needs another nudge.
- **Verification.** If a planned Verus lemma or Kani harness cannot be
  discharged after 6 hours of work on that single obligation, stop and
  escalate with the current proof state rather than weakening the obligation.
- **Ambiguity.** If a decision materially changes which source shapes the lint
  fires on, stop and present options with trade-offs.

## Risks

- **Risk: macro identity cannot be proven from an `EarlyContext`.**
  Severity: high. Likelihood: high.
  `rustc_lint::EarlyContext` exposes only `sess()`; it has no `TyCtxt`, so
  `DefId → crate name / definition path` resolution is not directly available.
  RFC 0002 §Detection architecture requires "resolved definition identifier,
  defining crate, and canonical definition path" to match an allowlist entry.
  Mitigation: `EP-M0` spike determines empirically what `ExpnData` yields;
  `DEC-002` records the three candidate architectures and the decision.

- **Risk: duplicate diagnostics from facade macros.**
  Severity: medium. Likelihood: medium.
  `tracing` with the `log` feature can emit the message through two paths, and
  each may lower to its own `ExprKind::FormatArgs` node carrying the same
  source literal span. Mitigation: deduplicate emitted diagnostics by the
  recovered literal span within one pass instance; assert this with a UI
  fixture that enables `tracing/log`.

- **Risk: `concat!()` rewrite changes the type in a coercion site.**
  Severity: medium. Likelihood: low.
  `"a"` and `concat!("a", "b")` are both `&'static str`, so the type is
  preserved, but a literal in a `const` pattern position or a `#[doc = "..."]`
  meta item is not an expression. Mitigation: the lint visits `check_expr`
  only, never patterns, attributes, or meta items (RFC 0002 §AST entry points).
  A UI fixture pins that a literal in a pattern is untouched.

- **Risk: suggestion applies incorrectly under `cargo fix`.**
  Severity: medium. Likelihood: low.
  A machine-applicable suggestion whose span is off by one quote character
  silently corrupts source. Mitigation: the applicability gate in
  `EP-M2`/`EP-M3` requires the recovered snippet to re-lex as exactly one
  cooked string token *and* the generated replacement to re-lex as exactly one
  `concat!` expression, before `MachineApplicable` is used. Non-vacuity control
  `NV-REWRITE-1` seeds a deliberate off-by-one and requires the check to reject
  it.

- **Risk: no existing lint in this repository emits a suggestion.**
  Severity: low. Likelihood: high (it is already true).
  Every current lint uses `cx.emit_span_lint(..., DiagDecorator(|lint| ...))`
  with `primary_message`/`span_note`/`span_label`/`help` only. `span_suggestion`
  exists on `rustc_errors::Diag` (verified, see `SURP-004`), but this lint will
  be the first user. Mitigation: `EP-M2` lands the suggestion path first, on
  the simplest possible case (a plain literal), so the mechanics are proven
  before the format-string complexity arrives.

- **Risk: `insta` snapshots of diagnostics duplicate the UI `.stderr` files.**
  Severity: low. Likelihood: medium.
  Mitigation: snapshots are used only for the *pure* rewrite output
  (fragment lists and the generated `concat!()` text across locales and
  multi-join shapes), not for rendered rustc diagnostics, which `.stderr`
  fixtures already pin.

- **Risk: Verus proof drifts from the production scanner.**
  Severity: medium. Likelihood: medium.
  Verus compiles its own files and mirrors production types
  (`docs/developers-guide.md` §Verus scope and trust boundary; existing proofs
  in `verus/` use `#[path = "../crates/..."]` includes where possible).
  Mitigation: include the production `continuation` domain module by `#[path]`
  where its dependencies allow, exactly as
  `verus/clone_detector_candidate_pair.rs` includes `fragment_id.rs`; where a
  spec mirror is unavoidable, state the trust boundary in the proof's module
  doc comment.

## Progress

- [x] (2026-08-21) Branch `2-2-10-string-continuation-style-post-expansion-early-lint.md`
      created from `harden-lint-config` and pushed with upstream tracking.
- [x] (2026-08-21) Reconnaissance of crate template, suite integration, i18n,
      and testing conventions completed; recorded in §Context and orientation.
- [x] (2026-08-21) Compiler API assumptions in RFC 0002 verified against
      `nightly-2026-05-28`; recorded in §Surprises & discoveries.
- [x] (2026-08-21) ExecPlan drafted.
- [ ] Plan approved by the user (required before any implementation).
- [ ] EP-M0 spike: pass architecture go/no-go.
- [ ] EP-M1 pure domain core.
- [ ] EP-M2 plain cooked string literals.
- [ ] EP-M3 source-authored format strings (compiler built-ins).
- [ ] EP-M4 `log` and `tracing` facade allowlist.
- [ ] EP-M5 suite, installer, and documentation integration.
- [ ] Roadmap item 2.2.10 marked done.

## Surprises & discoveries

These were found while validating RFC 0002 against the pinned toolchain,
before implementation started. They materially change parts of the RFC's
prescribed approach.

- **SURP-001: rustc already proves "the format string was written in source".**
  Observation: `rustc_ast::FormatArgs::is_source_literal` is computed by
  `rustc_parse_format::Parser::new`, which compares the recovered source
  snippet against the parsed input and returns `false` when a proc macro has
  respanned the literal (the fix for rust-lang/rust#114865).
  Evidence: `compiler/rustc_parse_format/src/lib.rs:302-345` and
  `compiler/rustc_builtin_macros/src/format.rs:290` in the
  `nightly-2026-05-28` `rustc-src` component.
  Impact: RFC 0002 §Format strings steps 3 and 4 ("recover an exact source
  literal span", "reject spans attributable only to an external or proc-macro
  expansion") are largely discharged by checking `is_source_literal`. The lint
  still re-lexes the snippet as a belt-and-braces check, but this is now
  defence in depth rather than the primary proof.

- **SURP-002: `FormatArgs::span` is the format-string literal token span.**
  Observation: `make_format_args` constructs `FormatArgs { span: fmt_span, .. }`
  where `fmt_span` comes from `expr_to_spanned_string`, which returns
  `span: expr.span` of the fully-expanded format-string expression — that is,
  the literal token including its quotes when `is_source_literal` holds.
  `Span::from_inner(InnerSpan)` maps a byte offset within that token to a
  source span (`compiler/rustc_span/src/lib.rs:1256`).
  Evidence: `compiler/rustc_builtin_macros/src/format.rs:170-181, 675-681`;
  `compiler/rustc_builtin_macros/src/util.rs:61-101`.
  Impact: per-continuation secondary labels and the whole-literal replacement
  span come directly from `FormatArgs::span`, offset by one for the opening
  quote. No bespoke span arithmetic over the enclosing macro call is needed.

- **SURP-003: `uncooked_fmt_str` carries the raw source body and is *not*
  newline-appended.**
  Observation: `FormatArgs::uncooked_fmt_str: (token::LitKind, Symbol)`, where
  the `Symbol` is `token_lit.symbol` — the raw, unescaped source text between
  the quotes. `println!`-family macros append `\n` to the *cooked* symbol
  (`fmt.symbol`) but leave `uncooked_symbol` untouched.
  Evidence: `compiler/rustc_ast/src/format.rs:44-59`;
  `compiler/rustc_builtin_macros/src/util.rs:61-101`;
  `compiler/rustc_builtin_macros/src/format.rs:190-192`.
  Impact: the scanner's input is `uncooked_fmt_str.1.as_str()` for format
  strings and `token::Lit::symbol` for plain literals. The
  `token::LitKind` discriminant (`Str` / `StrRaw(_)` / `ByteStr` / `CStr` /
  `CStrRaw(_)`) is exactly the cooked-versus-raw and type discriminator RFC
  0002 §Decision matrix needs.

- **SURP-004: `EarlyContext` implements `LintContext`; `Diag::span_suggestion`
  exists.**
  Observation: `impl LintContext for EarlyContext<'_>`
  (`compiler/rustc_lint/src/context.rs:619`) provides `sess()` and
  `emit_span_lint`. `Diag::span_suggestion(sp, msg, suggestion, applicability)`
  is available (`compiler/rustc_errors/src/diagnostic.rs:918`).
  Impact: the diagnostic path mirrors `bumpy_road_function`'s
  `cx.emit_span_lint(LINT, span, DiagDecorator(|lint| { ... }))`, adding
  `lint.span_suggestion(...)`. Source text comes from
  `cx.sess().source_map().span_to_snippet(span)`.

- **SURP-005: `LintStore::register_early_pass` exists and takes a
  `TyCtxt`-free factory.**
  Observation:
  `pub fn register_early_pass(&mut self, pass: impl Fn() -> EarlyLintPassObject + ...)`
  (`compiler/rustc_lint/src/context.rs:168`), alongside the public
  `early_passes` / `late_passes` fields.
  Impact: RFC 0002 §Suite integration's snippet compiles as written. The suite's
  existing behavioural assertion `then_late_pass_count` reads
  `store.late_passes.len()` and is unaffected; a new assertion on
  `store.early_passes.len()` is possible and should be added.

- **SURP-006: `EarlyContext` has no `TyCtxt`, so macro identity is not directly
  resolvable.**
  Observation: `ExpnData` gives `kind: ExpnKind::Macro(MacroKind, Symbol)`,
  `macro_def_id: Option<DefId>`, `def_site: Span`, `allow_internal_unstable`,
  and `parent_module` (`compiler/rustc_span/src/hygiene.rs:969-1015`), but
  turning a `DefId` into a crate name or definition path needs `TyCtxt`, which
  `EarlyContext` does not hold.
  Countervailing observation: early lints run inside a query —
  `fn early_lint_checks(tcx: TyCtxt<'_>, (): ())`
  (`compiler/rustc_interface/src/passes.rs:400`) — so a `TyCtxt` *does* exist
  on the thread, reachable via `rustc_middle::ty::tls`.
  Impact: this is the single largest open design question. See `DEC-002` and
  milestone `EP-M0`.

- **SURP-007: rustc normalizes CRLF out of source before lexing.**
  Observation: `normalize_src` calls `normalize_newlines`
  (`compiler/rustc_span/src/lib.rs:2510-2528`), so a `SourceFile`'s contents —
  and therefore every literal `Symbol` and every `span_to_snippet` result —
  contain LF only.
  Impact: RFC 0002 §Continuation scanner's CRLF and lone-CR branches are
  unreachable from the rustc adapter. Keep them in the *pure* scanner (they are
  cheap, they make the domain module self-contained, and the domain tests
  exercise them directly), but do not claim adapter-level coverage for them,
  and do not write a UI fixture that pretends to.

- **SURP-008: `googletest` and `pretty_assertions` are not yet dependencies.**
  Observation: neither appears in any `Cargo.toml` in the workspace nor in
  `[workspace.dependencies]`. Every existing test uses bare `assert_eq!`.
  `serial_test` is used pervasively for locale UI tests but is pinned
  per-crate, not in `[workspace.dependencies]`.
  Impact: see `DEC-006`.

- **SURP-009: `rustc-literal-escaper` is already in `Cargo.lock`.**
  Observation: `rustc-literal-escaper 0.0.4` is present transitively (pulled in
  by `ra_ap_syntax`). This is the crate rustc itself now uses for unescaping;
  it is no longer vendored inside `rustc_lexer`.
  Impact: it is the natural differential oracle for "the rewrite preserves the
  evaluated value". See `DEC-007`.

- **SURP-010: `log 0.4.33`'s facade macros capture `$($arg:tt)+`.**
  Observation: `log::info!` matches `(logger: $logger:expr, target: $target:expr, $($arg:tt)+)`,
  `(target: $target:expr, $($arg:tt)+)`, `(logger: $logger:expr, $($arg:tt)+)`,
  and `($($arg:tt)+)`, forwarding to `log!` and then `__log!`; key-value fields
  are separated from the message by `;`
  (`~/.cargo/registry/src/*/log-0.4.33/src/macros.rs:75-115, 252-270`).
  Impact: RFC 0002's `log` allowlist omits the newer `logger:` control. The
  allowlist must either add it or explicitly reject invocations carrying it.
  Because the captures are `tt`, a `concat!(...)` expression in the message
  position is grammatically accepted — but the lint must still *prove* it is
  looking at `log`'s macro, not a same-named local macro (`SURP-006`).

## Decision log

- **DEC-001: Implement as a post-expansion early lint, per RFC 0002.**
  Rationale: pre-expansion cannot see `FormatArgumentKind::Captured`; late HIR
  has lost the source spelling that distinguishes an escaped newline from an
  equivalent evaluated string. RFC 0002 §Alternatives considered records both
  rejections and this plan adopts them unchanged.
  Date/Author: 2026-08-21, planning agent.

- **DEC-002: Pass architecture for macro identity — OPEN, resolved at `EP-M0`.**
  Context: `SURP-006`. Three candidates:
  1. *Pure early pass, narrowed allowlist.* Prove identity only from
     `ExpnData` (`macro_def_id.is_local()`, `def_site` file, `ExpnKind`
     symbol, `allow_internal_unstable`). Cheapest; cannot satisfy RFC 0002's
     "defining crate and canonical definition path" clause; would force
     `log`/`tracing` out of the initial allowlist.
  2. *Early pass reaching `TyCtxt` through `rustc_middle::ty::tls::with_opt`.*
     Satisfies the RFC clause exactly, at the cost of a hidden global and a
     `rustc_middle` dependency. Must degrade to "no diagnostic" when the
     `TyCtxt` is absent.
  3. *Early collector plus late emitter*, mirroring Clippy's
     `FormatArgsStorage`: an `EarlyLintPass` records `FormatArgs` nodes keyed by
     span into shared storage; a `LateLintPass` (which has `TyCtxt`) resolves
     identity, handles plain literals from HIR, and emits every diagnostic.
     Most faithful to the RFC and to established prior art; costs a hand-written
     two-pass registration in the lint crate and in the suite.
  Recommendation to the reviewer: option 3 if `EP-M0` shows that
  `log`/`tracing` invocations reach the AST as `ExprKind::FormatArgs` with
  `is_source_literal == true`; otherwise option 1 with `log`/`tracing` deferred.
  Do not adopt option 2 without an explicit decision: reaching into thread-local
  compiler state is exactly the kind of hidden coupling this repository's
  hexagonal boundaries exist to prevent.
  Date/Author: 2026-08-21, planning agent. **Requires user decision or `EP-M0`
  evidence before `EP-M3` starts.**

- **DEC-003: Hexagonal split — domain modules take no rustc types.**
  The scanner (`continuation`), the policy (`classification`), and the
  rewriter (`rewrite`) are pure over `&str` and `Range<usize>`. The rustc
  adapter (`driver`) converts AST nodes into a `LiteralFacts` value and
  converts a `ContinuationDisposition` back into spans. The diagnostic adapter
  (`diagnostics`) converts a disposition into localized Fluent output.
  Rationale: `hexagonal-architecture` skill §Domain purity, and the practical
  benefit that the invariants worth proving (`INV-SCAN-*`, `LEM-REWRITE-1`) all
  live in the pure half, where `proptest`, Kani, and Verus can reach them
  without a compiler session.
  Date/Author: 2026-08-21, planning agent.

- **DEC-004: Keep CRLF handling in the pure scanner; do not claim adapter
  coverage.**
  Rationale: `SURP-007`. The pure scanner is a reusable byte-level component
  with a documented contract; the contract should be complete. But a UI fixture
  claiming to exercise CRLF would be theatre, since rustc normalizes it away.
  The verification plan records this as an explicit reachability bound.
  Date/Author: 2026-08-21, planning agent.

- **DEC-005: One diagnostic per literal, with one whole-literal replacement.**
  Rationale: RFC 0002 §Decision matrix, final row. Cascaded per-continuation
  suggestions would conflict under `cargo fix`.
  Date/Author: 2026-08-21, planning agent.

- **DEC-006: Add `googletest` and `pretty_assertions` to
  `[workspace.dependencies]`; promote `serial_test` there too.**
  Rationale: the task brief explicitly authorizes `googletest` and
  `pretty_assertions` and requires their use for clear test semantics; neither
  is present today (`SURP-008`). `serial_test` is already used by three lint
  crates with per-crate pins, and this crate needs it for locale UI tests, so
  promoting it removes a version-skew hazard. This is additive and touches only
  the workspace manifest and the new crate's manifest.
  Date/Author: 2026-08-21, planning agent.

- **DEC-007: Use `rustc-literal-escaper` as the value-preservation oracle —
  PROPOSED, requires approval.**
  Rationale: the central constraint is "the rewrite preserves the evaluated
  bytes". A property test asserting that against a reference unescaper written
  in the same pull request is weak — a shared misunderstanding of the escaping
  rules passes both. `rustc-literal-escaper` is the crate rustc itself uses and
  is already resolved in `Cargo.lock` (`SURP-009`), so this is a new direct
  edge, not a new subtree.
  Alternative if declined: write a spec-derived reference unescaper in the test
  module from the Rust Reference §String literals, and add three
  compile-and-run UI fixtures that `assert_eq!` the original literal against its
  rewritten form at runtime, which grounds the property in real compiler
  behaviour at a handful of points.
  Date/Author: 2026-08-21, planning agent. **Requires user decision before
  `EP-M1` starts.**

- **DEC-008: Add `logger:` to the `log` allowlist grammar, or reject it.**
  Rationale: `SURP-010`. RFC 0002 was written against an older `log` surface.
  Recommendation: reject invocations carrying `logger:` in the initial
  allowlist (they are rare, and rejecting is the conservative direction), and
  add a `pass_log_logger_control.rs` fixture asserting silence. Record the
  divergence from RFC 0002 §Detection architecture in the RFC itself at
  `EP-M4`.
  Date/Author: 2026-08-21, planning agent.

## Outcomes & retrospective

Not started. To be completed at each milestone boundary and at plan closure.
Before setting this plan to `COMPLETE`, reconcile every discovery against
`docs/rfcs/0002-string-continuation-style.md`: `SURP-001`, `SURP-002`,
`SURP-003`, `SURP-007`, and `SURP-010` all describe places where the RFC's
prescribed mechanics differ from the pinned toolchain's actual behaviour, and
the RFC must be amended rather than silently diverged from.

## Context and orientation

### What this repository is

Whitaker is a suite of [Dylint](https://github.com/trailofbits/dylint) lints for
Rust. Dylint loads compiled lint libraries into `rustc` at build time, so each
lint crate is a `cdylib` compiled against the pinned nightly compiler's private
crates (`rustc_ast`, `rustc_lint`, and so on).

Layout you need to know:

- `crates/<lint_name>/` — one directory per lint. Ten exist today.
- `crates/rustc_ast/`, `crates/rustc_lint/`, `crates/rustc_span/`,
  `crates/rustc_session/`, `crates/rustc_middle/`, `crates/clippy_utils/` —
  thin proxy crates that re-export the nightly compiler crates so lint crates
  do not each need `#![feature(rustc_private)]` plumbing. Depend on these by
  workspace alias (`rustc_ast = { workspace = true }`), never on the raw
  `extern crate`.
- `common/` — the `whitaker-common` crate: pure helpers plus the Fluent
  localization engine.
- `suite/` — `whitaker_suite`, the aggregated `cdylib` that bundles every lint.
- `installer/` — the `whitaker` CLI that builds, links, and stages lint
  libraries.
- `src/` — the top-level `whitaker` crate, which holds the CLI plus shared test
  harness code (`src/testing/ui/`) and the rustc-aware span helpers
  (`src/hir/`).
- `verus/` — Verus proof files, run out-of-band from Cargo.
- `docs/` — the knowledge base. Start at `docs/contents.md` and
  `docs/repository-layout.md`.

`rust-toolchain.toml` pins `nightly-2026-05-28` with the `rustc-dev` and
`rust-src` components.

### Terms used in this plan

- **Cooked string literal** — an ordinary `"..."` literal, in which backslash
  escape sequences are interpreted. Contrast a *raw* literal `r"..."`, where
  they are not.
- **Continuation escape** — a backslash immediately followed by a source
  newline inside a cooked literal, plus the whitespace the compiler then
  consumes. Its evaluated value is empty.
- **Source-line join** — a continuation escape with source content both before
  it on its physical line and after the whitespace it consumes. Its only
  purpose is to wrap one logical string across two source lines. This is what
  the lint targets.
- **Layout trim** — a continuation used to suppress a leading or trailing
  source newline and indentation rather than to divide two fragments, as in
  `"\` at the very start of a literal. The lint leaves these alone.
- **Post-expansion early lint** — an `EarlyLintPass` that runs on the abstract
  syntax tree (AST) after macro expansion but before lowering to the high-level
  intermediate representation (HIR). It sees `ExprKind::FormatArgs`, which
  records how `format!`-family arguments were bound, while still retaining the
  literal's source spelling.
- **Implicit format capture** — writing `format!("{name}")` so that `name` is
  captured from the enclosing scope. The compiler permits this only when the
  format string is a direct source literal, which is exactly why a `concat!()`
  rewrite would break it.
- **Applicability** — rustc's confidence label on a suggestion.
  `MachineApplicable` means `cargo fix` may apply it unattended.

### What exists today that you will copy

**Lint crate shape.** Use `crates/no_unwrap_or_else_panic/` as the template; it
is the closest existing multi-module lint. Its `Cargo.toml` declares
`[lib] crate-type = ["cdylib", "rlib"]`, `publish = false`, workspace-inherited
metadata, and two features:

```toml
[features]
default = []
dylint-driver = [
    "dep:whitaker-common", "dep:dylint_linting", "dep:fluent-templates",
    "dep:log", "dep:rustc_hir", "dep:rustc_lint", "dep:rustc_session",
    "dep:rustc_span", "dep:serde", "dep:whitaker",
]
constituent = ["dylint-driver", "dylint_linting/constituent"]
```

Every compiler dependency is `optional = true` so that plain `cargo check`,
`cargo doc`, and `cargo clippy` runs succeed without `rustc_private`. The
crate's `src/lib.rs` opens with
`#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]` and gates
every module on `dylint-driver`, with a `stub` module filling the crate when
the feature is off. The `crates/*` glob in the workspace `members` list picks
up a new directory automatically — no manifest edit is needed to register it.

**Lint declaration.** `crates/conditional_max_n_branches/src/driver.rs:73-91`
wraps the declaration macro in a private module so the macro-generated,
source-location-free items do not trip `missing_docs`:

```rust
mod declaration {
    #![expect(
        missing_docs,
        reason = "dylint_linting macro expansion emits items with no documentable source location"
    )]
    use super::ConditionalMaxNBranches;
    dylint_linting::impl_late_lint! {
        /// ...
        pub CONDITIONAL_MAX_N_BRANCHES,
        Warn,
        "...",
        ConditionalMaxNBranches::default()
    }
}
pub use declaration::CONDITIONAL_MAX_N_BRANCHES;
```

`dylint_linting` 6 provides `impl_early_lint!` with the same shape.

**No early lint exists yet.** Every current lint is a `LateLintPass`. This will
be the repository's first `EarlyLintPass`, so there is no in-repo precedent for
early-pass registration to copy; write it from `SURP-005`.

**Diagnostics.** `crates/bumpy_road_function/src/driver/diagnostic.rs:33-83` is
the model, because it is the only lint that emits per-item secondary labels:

```rust
let messages = safe_resolve_message_set(localizer, resolution, noop_reporter, || {
    fallback_messages(/* ... */)
});
cx.emit_span_lint(
    BUMPY_ROAD_FUNCTION,
    input.primary_span,
    rustc_lint::errors::DiagDecorator(|lint| {
        lint.primary_message(messages.primary().to_owned());
        lint.span_note(input.primary_span, messages.note().to_owned());
        for (ordinal, interval) in highlighted.iter().enumerate() { /* span_label */ }
        lint.help(messages.help().to_owned());
    }),
);
```

Note that `clippy_utils` is vendored but **unused** by every lint's logic; do
not reach for `span_lint_and_then`. Use `cx.emit_span_lint` with
`rustc_lint::errors::DiagDecorator`.

**Localization.** `whitaker_common::i18n` wraps `fluent_templates::static_loader!`
over `common/locales/{en-GB,cy,gd}/`. One `.ftl` file per lint, named after the
lint slug, with the message keyed by the slug and attributes `.note`, `.help`,
and (for labels) `.label`. A lint obtains its localizer once per run:

```rust
let shared_config = SharedConfig::load();
self.localizer = get_localizer_for_lint(LINT_NAME, shared_config.locale());
```

and resolves the primary/note/help triple with
`safe_resolve_message_set(&localizer, MessageResolution { lint_name, key, args }, noop_reporter, fallback)`.
Per-label strings use `localizer.attribute_with_args(LINT_NAME, "label", &args)`
and must have Fluent's bidirectional isolation marks (`U+2068`, `U+2069`) and
`U+FFFD` filtered out, exactly as `resolve_bump_label` does at
`crates/bumpy_road_function/src/driver/diagnostic.rs:175-189`.

**UI tests.** `crates/no_std_fs_operations/src/tests/ui.rs` is the locale-aware
template:

```rust
use serial_test::serial;
use whitaker_common::test_support::with_locale;

#[test] #[serial] fn ui() { run_with_locale("ui", None); }
#[test] #[serial] fn ui_runs_in_welsh() { run_with_locale("ui-cy", Some("cy")); }
#[test] #[serial] fn ui_runs_in_gaelic() { run_with_locale("ui-gd", Some("gd")); }
#[test] #[serial] fn ui_runs_in_fallback_locale() { run_with_locale("ui-fallback", Some("zz")); }

fn run_with_locale(directory: &str, locale: Option<&str>) {
    with_locale(locale, || {
        whitaker::run_ui_tests!(directory).expect("UI tests should execute without diffs");
    });
}
```

`#[serial]` is mandatory: `DYLINT_LOCALE` is process-global. Never mutate the
environment directly in a test; use `whitaker_common::test_support::{with_locale,
with_env_var, with_env_var_removed}` (`AGENTS.md` forbids direct mutation).

**Behavioural tests.** Feature files live at `<crate>/tests/features/*.feature`;
step definitions live in `<crate>/tests/<name>_behaviour.rs`. Scenarios bind by
crate-relative path and zero-based index:
`#[scenario(path = "tests/features/string_continuation_style.feature", index = 0)]`.
Fixtures carrying a "world" struct are wrapped in
`#[whitaker_test_macros::allow_fixture_expansion_lints]`, which works around
`rstest`'s macro expansion dropping `#[expect]` attributes.

**Suite registration.** `suite/src/lints.rs` holds two parallel, order-coupled
arrays: `SUITE_LINTS` (plain `LintDescriptor { name, crate_name }` metadata,
usable without the compiler) and `SUITE_LINT_DECLS` (`&[&rustc_lint::Lint]`).
`suite/tests/registration.rs::then_decls_align` asserts they agree, so the new
entry must go into both at the same index. `suite/src/driver.rs:65-68` is the
registration function to extend.

**Installer and Makefile.** The lint list is hard-coded in *two* places that
must stay in step: `installer/src/resolution.rs:17` (`LINT_CRATES`, a Rust
`&[&str]`) and `Makefile:58` (`LINT_CRATES`, a space-separated Make variable
that additionally contains `whitaker_suite`).

**Gates.** `make check-fmt`, `make typecheck`, `make lint`, `make test`,
`make markdownlint`, `make nixie`. `make test` runs `cargo nextest run` over the
workspace and then `cargo test --doc` with an exclusion list; doctests cannot
link `rustc_driver`, so the new crate is automatically excluded once it is
added to the Makefile's `LINT_CRATES`.

### The upstream specification

`docs/rfcs/0002-string-continuation-style.md` is the design of record. Read it
in full before starting. Its §Rule semantics, §Decision matrix, §Continuation
scanner, §Rewrite construction, and §Test plan are normative for this work,
except where §Surprises & discoveries above records a toolchain fact that
contradicts them.

### Skills and documents to load

Before starting implementation, load these skills (they encode conventions this
plan relies on rather than restates):

- `rust-router`, then `rust-unit-testing` and `rust-types-and-apis` for the
  domain modules.
- `hexagonal-architecture` for the port and adapter boundaries in `DEC-003`.
- `proptest` for the property tests in `EP-M1`.
- `kani` for the bounded harnesses in `EP-M1`.
- `verus` for `LEM-REWRITE-1`.
- `leta` for navigation; prefer `leta show`/`leta refs` over reading whole files.
- `execplans` for keeping this document current.
- `commit-message` when committing.
- `en-GB-oxendict` for all prose.

And these repository documents:

- `AGENTS.md` — the binding project policy layer.
- `docs/whitaker-dylint-suite-design.md` — suite conventions. Note that it is
  entirely late-pass oriented and contains no early-pass guidance; `EP-M5` adds
  that section.
- `docs/developers-guide.md` — internal conventions, including the Verus trust
  boundary rules.
- `docs/users-guide.md` §Available Lints — where the user-facing description goes.
- `docs/documentation-style-guide.md` — required for every doc edit.
- `docs/rust-testing-with-rstest-fixtures.md` and
  `docs/rstest-bdd-users-guide.md` — API mechanics. Both are adapted upstream
  material and cite paths that do not exist here; trust `AGENTS.md` and existing
  crate precedent over their file-path examples.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` — the scanner is
  a natural home for the very complexity this repository lints against; keep
  functions small and single-purpose.
- `docs/rust-doctest-dry-guide.md` — doc examples must be genuine, but note that
  this crate's doctests are excluded from `make test` because they cannot link
  `rustc_driver`. Put runnable examples on the *pure* domain functions, which
  are compiled without the `dylint-driver` feature; mark compiler-facing
  examples `ignore` with a reason, as `suite/src/driver.rs` does.

## Conformance basis

Upstream artefacts, at the revisions current on branch
`2-2-10-string-continuation-style-post-expansion-early-lint.md` (based on
`harden-lint-config` at `9b758d1`):

- **RFC-0002** — `docs/rfcs/0002-string-continuation-style.md`, status
  *Proposed*. The design of record.
- **ROADMAP-2.2.10** — `docs/roadmap.md` line 81. Depends on ROADMAP-2.1.1
  (lint crate template, done) and ROADMAP-2.3.4 (locale selection via
  `DYLINT_LOCALE` and `dylint.toml`, done).
- **SUITE-DESIGN** — `docs/whitaker-dylint-suite-design.md`. Governs crate
  layout, the `constituent` feature, and suite assembly. Contains no early-pass
  guidance; `EP-M5` must add it.
- **AGENTS** — `AGENTS.md`. Code style, testing policy, documentation
  maintenance, and the 400-line module ceiling.
- **STYLE** — `docs/documentation-style-guide.md`.

There is no Terms of Reference document for this work; ROADMAP-2.2.10 plus
RFC-0002 serve that role.

Trace links:

```plaintext
ROADMAP-2.2.10 -> RFC-0002 §Continuation scanner -> EP-M1 -> INV-SCAN-1 -> crates/string_continuation_style/src/continuation/tests.rs
ROADMAP-2.2.10 -> RFC-0002 §Rewrite construction -> EP-M1 -> LEM-REWRITE-1 -> verus/string_continuation_rewrite.rs
ROADMAP-2.2.10 -> RFC-0002 §Ordinary string expressions -> EP-M2 -> crates/string_continuation_style/ui/fail_plain_join.rs
ROADMAP-2.2.10 -> RFC-0002 §Why the motivating example passes -> EP-M3 -> crates/string_continuation_style/ui/pass_implicit_format_capture.rs
ROADMAP-2.2.10 -> RFC-0002 §Detection architecture (allowlist) -> EP-M4 -> crates/string_continuation_style/ui/pass_local_macro_lookalikes.rs
ROADMAP-2.2.10 -> RFC-0002 §Suite integration -> EP-M5 -> suite/tests/registration.rs::then_early_pass_count
ROADMAP-2.2.10 -> AGENTS §Documentation maintenance -> EP-M5 -> docs/users-guide.md §string_continuation_style
```

## Verification plan

The implementation structure in `DEC-003` was chosen *because* it puts every
obligation below into a pure module reachable without a compiler session. Each
obligation names the artefact that discharges it and the control that proves
the check is not vacuous.

### Axioms (assumed, not verified)

- **AX-1.** `rustc_ast::FormatArgs::is_source_literal == true` implies the
  format string was written as a direct literal in the source file and its
  recovered snippet matches the parsed input. Basis: `SURP-001`. This is a
  third-party interface contract; we do not verify rustc's internals. We *do*
  verify our repository-owned use of it: `EP-M3` includes UI fixtures for a
  `concat!()`-generated format string and an `include_str!()`-generated one,
  both of which must produce silence.
- **AX-2.** `FormatArgs::span` is the format-string literal token's span, and
  `Span::from_inner(InnerSpan::new(a, b))` yields the sub-span at byte offsets
  `a..b` from that token's start. Basis: `SURP-002`. Verified at the boundary by
  the byte-exact `.stderr` fixtures, whose caret positions would move if this
  were wrong.
- **AX-3.** `FormatArgs::uncooked_fmt_str.1` is the raw source body between the
  quotes, unmodified by the `println!` newline append. Basis: `SURP-003`.
  Boundary-verified by `fail_println_join.rs`, whose expected suggestion would
  gain a stray `\n` fragment if this were false.
- **AX-4.** Rust consumes, after a continuation escape's newline, every
  subsequent space, tab, and newline until the first other character. Basis:
  the Rust Reference §String literals, and rustc's `skip_ascii_whitespace`.
  This is the rule the scanner implements; `INV-SCAN-3` and its oracle test it.
- **AX-5.** `concat!()` yields a `&'static str` and, when it supplies a format
  string, disables implicit named capture. Basis: RFC-0002 §Source basis and
  rust-lang/rust's own UI test. Boundary-verified by
  `pass_implicit_format_capture.rs`.
- **AX-6.** rustc normalizes CRLF to LF before lexing, so adapter-supplied
  literal bodies contain LF only. Basis: `SURP-007`.
- **AX-7.** `LintStore::register_early_pass` causes the pass to run once per
  crate, after expansion, before HIR lowering. Basis: `SURP-005` and
  `compiler/rustc_interface/src/passes.rs:400`.

### Obligations

**INV-SCAN-1 — continuation detection is backslash-parity-correct.**

- Statement: a byte offset `i` in the literal body begins a continuation escape
  if and only if the byte at `i` is `\`, the maximal run of consecutive
  backslashes ending at `i` has odd length, and the byte at `i + 1` is `\n` (or
  begins a `\r\n` pair).
- Method: `rstest` parameterized cases for the finite boundary partition
  (runs of length 0 through 4, before LF, before CRLF, before a lone CR, before
  end-of-body), plus a Kani harness for exhaustive coverage of short inputs.
- Rationale: the parity rule is where a naive implementation goes wrong, and
  the input alphabet that matters is tiny, so bounded exhaustive exploration is
  both cheap and complete over the interesting domain.
- Domain: Kani harness over bodies of length ≤ 8 drawn from the four-symbol
  alphabet `{ '\\', '\n', ' ', 'a' }`, with `#[kani::unwind(10)]`.
- Artefact: `crates/string_continuation_style/src/continuation/tests.rs`
  (parameterized) and `crates/string_continuation_style/src/continuation/kani.rs`
  (`#[cfg(kani)]`).
- Evidence: `cargo nextest run -p string_continuation_style continuation::` and
  `scripts/run-kani.sh string-continuation`. Both must fail before the scanner
  exists.
- Non-vacuity: witnesses `"a\\\nb"` (odd run, is a continuation) and
  `"a\\\\\nb"` (even run, is a literal backslash followed by a real newline)
  must both be exercised and must be classified differently. Negative control
  `NV-SCAN-1`: replace the parity test with `is_backslash(prev)` and require the
  Kani harness to produce a counter-example naming the even-run input.

**INV-SCAN-2 — recorded ranges are well-formed.**

- Statement: for every `Continuation` the scanner emits,
  `escape_range.start < escape_range.end <= skipped_whitespace_range.start <=
  skipped_whitespace_range.end <= body.len()`; consecutive continuations are
  strictly ordered; and no two continuations' combined
  `escape_range ∪ skipped_whitespace_range` intervals overlap.
- Method: `proptest`, asserted on every scan in every property test via a
  shared `assert_scan_well_formed` helper, plus the same assertion inside the
  Kani harness for `INV-SCAN-1`.
- Rationale: this is a structural invariant over generated inputs of
  unbounded length; a property test is the proportionate tool, and folding it
  into every other property test costs nothing.
- Domain: generated bodies mixing ASCII text, Unicode text, escaped quotes,
  escaped backslashes, blank lines, and one to five continuations.
- Artefact: `crates/string_continuation_style/src/continuation/props.rs`.
- Evidence: `cargo nextest run -p string_continuation_style continuation::props`.
- Non-vacuity: the generator must be classified with `proptest::prop_assume!`-free
  construction and a `prop_assert!(scan.len() >= 1)` guard, so a generator that
  silently stopped producing continuations fails loudly rather than passing
  vacuously. Negative control `NV-SCAN-2`: make the scanner emit
  `skipped_whitespace_range = escape_range` and require the disjointness or
  ordering assertion to reject it.

**INV-SCAN-3 — the skipped-whitespace range matches the language rule.**

- Statement: `skipped_whitespace_range` covers exactly the bytes rustc
  discards after the continuation's newline — every space, tab, LF, and
  complete CRLF — terminating at the first other byte or at the end of the body.
- Method: differential property test against an independent unescaper
  (`DEC-007`), plus `rstest` cases for the boundaries (immediately-following
  content, blank lines, whitespace-only trailing run, end of body).
- Rationale: this is the obligation most likely to be got subtly wrong, and it
  is the one where an independent oracle is available. Examples alone would not
  cover the interaction of blank lines with indentation.
- Domain: as `INV-SCAN-2`, with the added generator requirement that at least
  one continuation is followed by non-empty indentation.
- Artefact: `crates/string_continuation_style/src/continuation/props.rs`.
- Evidence: as above.
- Non-vacuity: the generator must emit non-empty indentation after every
  newline, so that an implementation deleting only `escape_range` and retaining
  the indentation fails. Negative control `NV-SCAN-3`: that exact mutation must
  be rejected with a diff showing the retained spaces.

**INV-CLASS-1 — classification is total and mutually exclusive.**

- Statement: every continuation receives exactly one of `Join`,
  `LeadingLayoutTrim`, `TrailingLayoutTrim`; `Join` holds if and only if there
  is source content before the backslash on its physical line *and* source
  content after all consumed whitespace; when neither side has content,
  `LeadingLayoutTrim` wins.
- Method: `rstest` parameterized cases covering the 2×2 content matrix plus the
  precedence tie, and a Kani harness reusing the `INV-SCAN-1` symbolic input to
  assert exclusivity exhaustively over short bodies.
- Rationale: a finite partition with an explicit precedence rule. Both an
  enumerated table and bounded exhaustive exploration are cheap; together they
  cover the named cases and the unnamed ones.
- Domain: the five rows of RFC-0002 §Continuation scanner's classification
  table, plus Kani over bodies of length ≤ 8.
- Artefact: `crates/string_continuation_style/src/classification/tests.rs` and
  `crates/string_continuation_style/src/continuation/kani.rs`.
- Evidence: `cargo nextest run -p string_continuation_style classification::`.
- Non-vacuity: each of the three variants must be produced by at least one
  case, and the Kani assertion must be `exactly_one_of(...)`, not
  `at_least_one_of(...)`. Negative control `NV-CLASS-1`: swap the precedence so
  `TrailingLayoutTrim` wins the tie, and require the precedence case to fail.

**LEM-REWRITE-1 — splitting and re-concatenating equals removal.**

- Statement: let `body` be the literal body and `R = [r_0, ..., r_{n-1}]` a
  sorted, pairwise-disjoint sequence of sub-ranges of `body`. Let `fragments`
  be the `n + 1` maximal sub-sequences of `body` lying between and outside the
  ranges of `R`. Then `concat(fragments) == body with every r_i removed`.
- Method: Verus deductive proof over `Seq<u8>`, by induction on `|R|`.
- Rationale: this is the lemma the entire suggestion rests on, it must hold for
  literals of every length and every number of joins, and it is a pure
  statement about sequences — precisely the unbounded, algebraic shape Verus is
  for. A property test would sample it; a proof settles it.
- Domain: all `Seq<u8>` and all sorted disjoint range sequences. Unbounded.
- Artefact: `verus/string_continuation_rewrite.rs`, following the layout of
  `verus/clone_detector_candidate_pair.rs` — mirror the production range type
  by `#[path]` include where the module's dependencies permit, and state the
  trust boundary in the module doc comment otherwise.
- Evidence: `make verus` (add a `string-continuation` group to
  `scripts/run-verus.sh`). Before the proof body is written, the lemma must
  fail with an open goal, not vacuously succeed.
- Non-vacuity: inspect that the antecedent is inhabited by exhibiting a
  concrete witness (`body = "abc"`, `R = [1..2]`, fragments `["a", "c"]`)
  inside the proof file as an `assert` that Verus must discharge from the
  lemma. The proof must not contain `assume`. The `decreases` clause must be
  present and the base case must be `|R| == 0`, so the induction does work
  rather than collapsing. Negative control `NV-REWRITE-2`: weaken the
  disjointness precondition and confirm Verus rejects the proof rather than
  accepting it.

**LEM-REWRITE-2 — the rewrite preserves the evaluated value.**

- Statement: for a body whose selected joins are `J`, unescaping the original
  body yields the same character sequence as unescaping each rewrite fragment
  and concatenating the results.
- Method: differential `proptest` against the reference unescaper of `DEC-007`,
  plus three compile-and-run UI fixtures that assert the equality at runtime in
  real compiled Rust.
- Rationale: `LEM-REWRITE-1` is about source bytes; this is about *evaluated*
  bytes, and the bridge between them is the escaping rule, which is external
  (`AX-4`). Verifying against an independent implementation of that rule, and
  then grounding a few points in actual compiler output, covers both the
  general case and the "did we understand the rule at all" case.
- Domain: generated bodies containing Unicode above the basic multilingual
  plane, escaped quotes, escaped backslashes, `\u{...}` escapes, doubled braces
  (`{{`, `}}`), and one to five joins.
- Artefact: `crates/string_continuation_style/src/rewrite/props.rs` and
  `crates/string_continuation_style/ui/fail_value_preserved.rs` (a fixture whose
  fixed output, once applied, is compiled and run).
- Evidence: `cargo nextest run -p string_continuation_style rewrite::props`.
- Non-vacuity: classify generated cases by join count and by whether any
  Unicode scalar above `U+FFFF` appears, and require every class to be hit
  (`proptest`'s statistics, checked by an explicit counter assertion at the end
  of the run rather than by eyeball). Negative control `NV-REWRITE-3`: make the
  rewriter split at a `LeadingLayoutTrim` as well as at joins, and require the
  property to fail with a fragment boundary inside the trimmed region.

**LEM-APPLY-1 — machine-applicable implies re-lexable.**

- Statement: the lint labels a suggestion `MachineApplicable` only when the
  original snippet lexes as exactly one cooked string token and the generated
  replacement lexes as exactly one `concat!` macro-call expression.
- Method: `rstest` parameterized cases over the applicability gate, driven
  through the pure gate function, plus the UI matrix as an end-to-end check.
- Rationale: this is a finite conjunction of checks; enumerating the ways each
  conjunct can fail is proportionate. The real risk is that the gate is never
  consulted, which the negative control catches.
- Domain: the seven conditions in RFC-0002 §Applicability.
- Artefact: `crates/string_continuation_style/src/rewrite/tests.rs`.
- Evidence: `cargo nextest run -p string_continuation_style rewrite::`.
- Non-vacuity: each of the seven conditions must have a case that fails it
  alone, with all others satisfied, and each such case must downgrade the
  applicability. Negative control `NV-APPLY-1`: hard-code the gate to return
  `MachineApplicable` and require all seven cases to fail.

**Obligations deliberately not verified formally.** The macro-identity
allowlist (`EP-M4`) is a policy table, not an invariant: its correctness is a
statement about third-party crates' macro definitions, which change between
their versions. It is verified by UI fixtures pinning both the accepted forms
and the lookalikes, and by the "uncertain contract produces neither a
diagnostic nor a suggestion" default, which is the safe direction. This is
recorded here rather than omitted, because a reader could reasonably expect a
proof and should know why there is not one.

## Plan of work

### Stage A — understand and propose (no code changes)

Read RFC-0002 in full. Read `AGENTS.md`. Confirm `DEC-002` and `DEC-007` have
been decided. Do not write code until this plan is approved.

### Stage B — red tests and the behaviour specification

Write the feature file and the failing tests before any production code. The
feature specification that drives `EP-M1` and `EP-M2` is, at
`crates/string_continuation_style/tests/features/string_continuation_style.feature`:

```gherkin
Feature: Context-sensitive string continuation style

  Scenario: An ordinary source-line join prefers concat
    Given a cooked string expression with an interior continuation
    And the literal is in a general expression context
    When the continuation is classified
    Then the disposition is PreferConcat

  Scenario: An implicit format capture requires the direct literal
    Given a source-authored format string with an interior continuation
    And rustc classified one argument as Captured
    When the continuation is classified
    Then the disposition is RequireContinuation with reason ImplicitFormatCapture

  Scenario: A leading continuation expresses source layout
    Given a cooked string whose first token after the opening quote is a continuation
    When the continuation is classified
    Then the disposition is RequireContinuation with reason LeadingOrTrailingLayoutTrim

  Scenario: A byte string keeps its continuation
    Given a cooked byte string expression with an interior continuation
    When the continuation is classified
    Then the disposition is RequireContinuation with reason NonStringLiteralType

  Scenario: A raw string is not the lint's business
    Given a raw string expression containing a backslash before a newline
    When the continuation is classified
    Then the disposition is Ignore

  Scenario: Several joins produce one rewrite
    Given a cooked string expression with three interior continuations
    When the continuation is classified
    Then the disposition is PreferConcat
    And the rewrite has four fragments
```

Each stage below ends with `make check-fmt && make typecheck && make lint &&
make test`. Do not proceed while any gate fails.

### Stage C — implementation with verification developed alongside

Milestones `EP-M0` through `EP-M4`, in order. Each writes its Kani harness or
Verus lemma in the same commit as the code it constrains, not afterwards.

### Stage D — refactor, documentation, and wider validation

Milestone `EP-M5`: suite registration, installer and Makefile lists, user
guide, developers' guide, suite design addendum, RFC amendments for
`SURP-001`/`SURP-002`/`SURP-003`/`SURP-007`/`SURP-010`, and the roadmap tick.

## Milestones and plateaus

### EP-M0 — Architecture spike (prototyping)

- **Outcome.** A throwaway branch-local experiment that answers `DEC-002`, then
  is deleted. Repository state at the end of the milestone is unchanged except
  for this plan's `Decision log` and `Surprises & discoveries`.
- **Requirements advanced.** RFC-0002 §Detection architecture.
- **Method.** Create a scratch lint crate registering an `EarlyLintPass` that,
  for every `ExprKind::FormatArgs` and `ExprKind::Lit`, prints: the expression
  span, `span.ctxt().outer_expn_data()`'s `kind`, `macro_def_id`, and
  `def_site` filename; `is_source_literal`; `uncooked_fmt_str.0`; and the
  argument kinds. Run it over a fixture crate that uses `format!`, `println!`,
  `write!`, `assert!`, `log::info!`, `log::warn!(target: ...)`,
  `tracing::info!` with fields, `tracing::info!` with controls, a local
  `macro_rules!` named `info`, and a local macro binding `$msg:literal`.
- **Acceptance evidence.** A transcript in this plan's §Artefacts and notes
  showing, for each fixture line, whether a `FormatArgs` node appeared, whether
  `is_source_literal` was true, whether duplicate nodes appeared for one source
  literal, and what identity information `ExpnData` carried. Evidence
  identifier `EV-M0-transcript`.
- **Go/no-go.** If `log` and `tracing` invocations reach the AST as
  `ExprKind::FormatArgs` with `is_source_literal == true` **and** `ExpnData`
  alone cannot distinguish `log::info!` from a local `info!`, adopt `DEC-002`
  option 3 and revise `EP-M3`/`EP-M4` accordingly. If they do not appear as
  `FormatArgs` at all, adopt option 1 and move `log`/`tracing` support out of
  this plan into a new roadmap item, recording the deviation in `Decision log`
  and setting this plan to `BLOCKED` pending acceptance.
- **Conformance check.** Does the evidence falsify RFC-0002's assumption that a
  pure early pass suffices? If so, the RFC must be amended before `EP-M3`.
- **Recovery.** `git checkout -- .` and delete the scratch crate; nothing is
  committed from this milestone except plan updates.
- **Remaining gaps.** All implementation.
- **Compatibility decision.** None required; nothing ships.

### EP-M1 — Pure domain core

- **Outcome.** `crates/string_continuation_style/` exists and builds under both
  feature configurations. The three pure modules — `continuation` (scanner),
  `classification` (policy), `rewrite` (fragment construction and the
  applicability gate) — are complete, documented, and fully covered. The lint is
  declared and an `EarlyLintPass` is wired, but `check_expr` does nothing yet.
  `cargo dylint --lib string_continuation_style` runs and reports nothing.
- **Requirements discharged.** RFC-0002 §Continuation scanner, §Rewrite
  construction. `INV-SCAN-1`, `INV-SCAN-2`, `INV-SCAN-3`, `INV-CLASS-1`,
  `LEM-REWRITE-1`, `LEM-REWRITE-2`, `LEM-APPLY-1`.
- **Acceptance evidence.** `EV-M1`: `cargo nextest run -p string_continuation_style`
  passes; `scripts/run-kani.sh string-continuation` discharges both harnesses;
  `make verus` discharges `verus/string_continuation_rewrite.rs`; every
  negative control `NV-*` has been run and observed to fail for the stated
  reason, with the transcript recorded in §Artefacts and notes.
- **Conformance check.** Domain modules import no `rustc_*` crate — verify with
  `leta refs` and by confirming the modules compile with `--no-default-features`.
  Every module is under 400 lines. No public interface outside this crate
  changed except the three workspace dependency additions of `DEC-006`.
- **Recovery.** The crate is self-contained and unregistered; deleting the
  directory and reverting the workspace manifest returns the repository to its
  prior state.
- **Remaining gaps.** No AST is inspected; no diagnostic is emitted.
- **Compatibility decision.** None. This is a new pre-1.0 crate with no
  consumers.

### EP-M2 — Plain cooked string literals

- **Outcome.** `check_expr` handles `ExprKind::Lit` with a cooked
  `token::LitKind::Str`. A plain source-line join produces one warning with a
  note, a help, one secondary label per join, and a `MachineApplicable`
  whole-literal `concat!()` suggestion. Byte strings, C strings, raw strings,
  layout trims, and literals from macro expansions produce silence. Diagnostics
  are localized in `en-GB`, `cy`, and `gd`.
- **Requirements discharged.** RFC-0002 §Ordinary string expressions,
  §Diagnostic, and the plain-literal rows of §Decision matrix.
- **Acceptance evidence.** `EV-M2`: the UI fixtures `fail_plain_join`,
  `fail_multiple_joins`, `fail_join_across_blank_lines`,
  `fail_value_preserved`, `pass_leading_layout_trim`,
  `pass_trailing_layout_trim`, `pass_byte_and_c_strings`, `pass_raw_strings`,
  `pass_real_newline`, `pass_literal_in_pattern`, and
  `pass_unknown_literal_macro` all pass with byte-exact `.stderr`; `ui-cy`,
  `ui-gd`, and `ui-fallback` locale smoke fixtures pass. The behavioural
  scenarios for `PreferConcat`, `LeadingOrTrailingLayoutTrim`,
  `NonStringLiteralType`, `Ignore`, and the multi-join rewrite all pass.
- **Conformance check.** Confirm the suggestion span covers the whole literal
  including quotes and that applying it by hand to `fail_plain_join.rs`
  produces source that compiles and evaluates identically. Confirm no lint
  fires on any pattern, attribute, or meta-item literal.
- **Recovery.** Revert the `check_expr` body to the `EP-M1` no-op; the crate
  returns to a coherent silent state.
- **Remaining gaps.** Format strings are not inspected.
- **Compatibility decision.** None.

### EP-M3 — Source-authored format strings from compiler built-ins

- **Outcome.** `check_expr` also handles `ExprKind::FormatArgs`. Format strings
  from `format_args!`, `format!`, `print`/`println`/`eprint`/`eprintln`,
  `write`/`writeln`, `panic`/`todo`/`unimplemented`/`unreachable`, and the
  custom-message branches of `assert`/`debug_assert`/`assert_eq`/`assert_ne`/
  `debug_assert_eq`/`debug_assert_ne` are diagnosed when — and only when — the
  format string is a source literal, contains no `FormatArgumentKind::Captured`
  argument, and the macro identity is proven per the `EP-M0` decision.
- **Requirements discharged.** RFC-0002 §Format strings, §Why the motivating
  example passes, and the format-string rows of §Decision matrix.
- **Acceptance evidence.** `EV-M3`: the UI fixtures `fail_positional_arguments`,
  `fail_explicit_format_arguments`, `fail_println_join`, `fail_write_join`,
  `fail_assert_message_join`, `pass_implicit_format_capture` (the exact
  pull-request-296 example, which must remain clean), `pass_implicit_width_capture`,
  `pass_implicit_precision_capture`, and `pass_generated_format_string` all pass.
  The RFC's "must fail with a machine-applicable rewrite" example produces
  exactly the RFC's expected replacement, pinned by an `insta` snapshot of the
  generated `concat!()` text.
- **Conformance check.** Re-read RFC-0002 §Why the motivating example passes
  and confirm the implemented predicate is `any(|a| matches!(a.kind,
  Captured(_)))` over `arguments.all_args()`, catching captured width and
  precision. Confirm no diagnostic is emitted for any macro whose identity was
  not proven.
- **Recovery.** Remove the `FormatArgs` arm; `EP-M2` behaviour is intact.
- **Remaining gaps.** `log` and `tracing` are not yet in the allowlist.
- **Compatibility decision.** None.

### EP-M4 — `log` and `tracing` facade allowlist

*Conditional on the `EP-M0` go/no-go.*

- **Outcome.** The allowlist admits `log::{trace, debug, info, warn, error}!`
  in the `(format, arguments...)` and `(target: expr, format, arguments...)`
  forms, and `tracing::{trace, debug, info, warn, error}!` with a message tail
  after validated `target:`/`parent:`/`name:` controls and structured fields.
  Every other variant, every same-named local macro, and every same-named macro
  from an unapproved crate produce silence. Invocations carrying `log`'s
  `logger:` control are rejected (`DEC-008`).
- **Requirements discharged.** RFC-0002 §Detection architecture allowlist and
  §Acceptance fixtures.
- **Acceptance evidence.** `EV-M4`: `fail_log_message`, `fail_log_target`,
  `fail_tracing_fields`, `fail_tracing_controls`, `pass_log_implicit_capture`,
  `pass_log_target_implicit_capture`, `pass_log_kv_fields`,
  `pass_log_logger_control`, `pass_tracing_implicit_capture`,
  `pass_tracing_controls_implicit_capture`, `pass_tracing_no_message`,
  `pass_local_macro_lookalikes`, and `pass_external_macro_lookalikes` all pass.
  A `pass_tracing_with_log_feature` fixture proves exactly one diagnostic is
  emitted when `tracing/log` is enabled.
- **Conformance check.** Confirm the identity proof matches the architecture
  chosen at `EP-M0`. Confirm the divergence from RFC-0002's `log` grammar
  (`DEC-008`) has been written back into the RFC.
- **Recovery.** Remove the two facade entries from the allowlist table; the
  built-in coverage from `EP-M3` is unaffected.
- **Remaining gaps.** Suite integration.
- **Compatibility decision.** None.

### EP-M5 — Suite, installer, and documentation integration

- **Outcome.** `whitaker_suite` registers the early pass alongside its combined
  late pass. `cargo dylint list` shows `string_continuation_style` in both the
  standalone and suite libraries. Documentation is complete and the roadmap item
  is ticked.
- **Requirements discharged.** RFC-0002 §Suite integration; AGENTS
  §Documentation maintenance; ROADMAP-2.2.10.
- **Acceptance evidence.** `EV-M5`: `suite/tests/registration.rs` passes,
  including a new `then_early_pass_count` step asserting
  `store.early_passes.len() == 1` and a matching scenario line in
  `suite/tests/features/suite_registration.feature`;
  `make publish-check` succeeds; `make markdownlint` and `make nixie` pass.
- **Conformance check.** Both `LINT_CRATES` lists agree. `SUITE_LINTS` and
  `SUITE_LINT_DECLS` are index-aligned. Every discovery in §Surprises &
  discoveries has been reconciled with RFC-0002. No unapproved dependency,
  trust boundary, or persisted-format change was introduced.
- **Recovery.** Revert the suite and installer edits; the standalone lint crate
  remains usable on its own.
- **Remaining gaps.** None. Plan closes.
- **Compatibility decision.** None. Whitaker is pre-1.0 (`version = "0.2.7"`)
  and the suite's lint list is not an external commitment.

## Concrete steps

All commands run from the repository root,
`/home/leynos/.lody/repos/github---leynos---whitaker/worktrees/79835686-da6f-44b3-8bc2-5c7544bdeec4`,
on branch `2-2-10-string-continuation-style-post-expansion-early-lint.md`.

Log every gate run to a file so truncated output can be reviewed:

```bash
make test 2>&1 | tee "/tmp/test-whitaker-$(git branch --show-current).out"
```

### Step 1 — workspace dependency additions (`DEC-006`)

Edit `Cargo.toml` `[workspace.dependencies]`, adding:

```toml
googletest = "0.14"
pretty_assertions = "1"
serial_test = "3"
```

Pin `googletest` and `serial_test` to whatever `cargo add --dry-run` reports as
current and compatible with `rstest 0.26.1`; record the resolved versions here
when the step is done. Then update the three crates that pin `serial_test`
locally (`function_attrs_follow_docs`, `module_must_have_inner_docs`,
`no_std_fs_operations`) to `serial_test = { workspace = true }`.

Expected:

```plaintext
$ cargo metadata --format-version 1 >/dev/null && echo ok
ok
```

Commit: `Add googletest, pretty_assertions, and serial_test workspace pins`.

### Step 2 — scaffold the crate

Copy the shape of `crates/no_unwrap_or_else_panic/`. Create:

```plaintext
crates/string_continuation_style/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── classification/{mod.rs,tests.rs}
│   ├── continuation/{mod.rs,kani.rs,props.rs,tests.rs}
│   ├── diagnostics.rs
│   ├── driver.rs
│   ├── facts.rs
│   ├── rewrite/{mod.rs,props.rs,tests.rs}
│   ├── lib_ui_tests.rs
│   └── tests/{mod.rs,behaviour.rs,ui.rs}
├── tests/features/string_continuation_style.feature
├── ui/            ui-cy/            ui-gd/            ui-fallback/
```

The `crates/*` glob in the workspace `members` list picks the directory up
automatically. Verify:

```plaintext
$ cargo check -p string_continuation_style
    Finished `dev` profile
$ cargo check -p string_continuation_style --features dylint-driver
    Finished `dev` profile
```

Commit: `Scaffold the string_continuation_style lint crate`.

### Step 3 — EP-M1, red first

Write `crates/string_continuation_style/src/continuation/tests.rs` with the
`INV-SCAN-1` boundary cases before writing the scanner. Confirm the red:

```plaintext
$ cargo nextest run -p string_continuation_style continuation::
error[E0433]: failed to resolve: use of undeclared crate or module `continuation`
```

Then implement, and confirm green. Repeat for `classification` and `rewrite`.
Add the Kani harness and the Verus proof in the same commits as the code they
constrain. Run each negative control once, record the observed failure in
§Artefacts and notes, then revert it.

### Step 4 — EP-M2 through EP-M4

For each UI fixture: write the `.rs` file first, run the UI test, inspect the
produced diff, and only then create the `.stderr` from the observed output.
`dylint_testing` layers on `trybuild`, so the blessing mechanism is
`TRYBUILD=overwrite`; confirm this on the first fixture before relying on it,
and if it does not apply, transcribe the diff by hand rather than guessing.

```bash
TRYBUILD=overwrite cargo nextest run -p string_continuation_style ui 2>&1 \
  | tee "/tmp/ui-whitaker-$(git branch --show-current).out"
```

Never bless a `.stderr` without reading it. A blessed-but-wrong expectation is
worse than a failing test.

### Step 5 — EP-M5 integration

Edit, in this order:

1. `suite/Cargo.toml` — add `dep:string_continuation_style` to the
   `dylint-driver` feature and the path dependency with
   `features = ["dylint-driver", "constituent"]`.
2. `suite/src/lints.rs` — append to `SUITE_LINTS` and `SUITE_LINT_DECLS` at the
   same index, and extend the doctest's expected name list.
3. `suite/src/driver.rs` — insert the `store.register_early_pass(...)` call
   into `register_suite_lints`, between `register_lints` and
   `register_late_pass`. Update the doctest's `get_lints().len()` from 9 to 10.
4. `suite/tests/features/suite_registration.feature` and
   `suite/tests/registration.rs` — add the early-pass count scenario and step.
5. `installer/src/resolution.rs` — add `"string_continuation_style"` to
   `LINT_CRATES`.
6. `Makefile` — add `string_continuation_style` to the `LINT_CRATES` variable.
7. `common/locales/{en-GB,cy,gd}/string_continuation_style.ftl` — the message,
   `.note`, `.help`, and `.label` entries.
8. `README.md` — the lint table row, and the "ships nine standard lints" count.
9. `docs/users-guide.md` — a `### string_continuation_style` subsection under
   §Available Lints covering what fires, what does not, and why.
10. `docs/developers-guide.md` — a short section on writing early passes in this
    repository: the `EarlyContext` limitations from `SURP-006`, the
    `is_source_literal` guarantee from `SURP-001`, and the locale-serialization
    requirement.
11. `docs/whitaker-dylint-suite-design.md` — an addendum documenting early-pass
    registration in the suite.
12. `docs/rfcs/0002-string-continuation-style.md` — amendments for `SURP-001`,
    `SURP-002`, `SURP-003`, `SURP-007`, and `SURP-010`; move status from
    *Proposed* to *Accepted*.
13. `docs/roadmap.md` — tick item 2.2.10.

Then:

```plaintext
$ make publish-check 2>&1 | tee "/tmp/publish-check-whitaker-$(git branch --show-current).out"
...
$ cargo dylint list | grep string_continuation_style
string_continuation_style
```

## Validation and acceptance

Run all four gates after every milestone, sequentially, never in parallel — the
build cache makes sequential runs faster overall:

```bash
make check-fmt 2>&1 | tee "/tmp/check-fmt-whitaker-$(git branch --show-current).out"
make typecheck 2>&1 | tee "/tmp/typecheck-whitaker-$(git branch --show-current).out"
make lint      2>&1 | tee "/tmp/lint-whitaker-$(git branch --show-current).out"
make test      2>&1 | tee "/tmp/test-whitaker-$(git branch --show-current).out"
```

Delegate full gate runs to the `scrutineer` subagent, which runs them
sequentially, captures each to a log, and returns a bounded report. When it
reports a failure, read the cited log rather than re-running the gate.

Red-Green-Refactor evidence to record for each domain module:

- **Red.** `cargo nextest run -p string_continuation_style <module>::` fails
  with an unresolved-item or assertion error naming the missing behaviour.
- **Green.** The same command passes after the minimal implementation.
- **Refactor.** The same command, then the four gates, all pass after cleanup.

Behaviour-driven evidence:

- **Red.** `cargo nextest run -p string_continuation_style behaviour` fails
  because the step definitions are unimplemented.
- **Green.** The same command passes; each scenario in
  `tests/features/string_continuation_style.feature` maps to one `#[scenario]`.

Verification evidence:

- `scripts/run-kani.sh string-continuation` reports both harnesses
  `VERIFICATION:- SUCCESSFUL`, with the explored unwind bound recorded.
- `make verus` reports `verification results:: N verified, 0 errors` for
  `verus/string_continuation_rewrite.rs`.
- Every `NV-*` negative control has been run and its failure transcript
  recorded in §Artefacts and notes.

Quality criteria — what "done" means:

- **Tests.** `make test` passes with the whole workspace green, including the
  new crate's unit, property, behavioural, snapshot, and UI suites in four
  locales.
- **Verification.** `INV-SCAN-1`, `INV-SCAN-2`, `INV-SCAN-3`, `INV-CLASS-1`,
  `LEM-REWRITE-1`, `LEM-REWRITE-2`, and `LEM-APPLY-1` are all discharged by the
  named artefacts, with non-vacuity controls observed.
- **Lint and typecheck.** `make check-fmt`, `make typecheck`, and `make lint`
  all exit zero. Whitaker lints itself clean, including `module_max_lines`.
- **Documentation.** `make markdownlint` and `make nixie` pass.
- **Performance.** No benchmark threshold. The scanner is linear in literal
  length and runs once per string literal; if `make test` wall-clock grows by
  more than 10 per cent, investigate before closing the plan.
- **Security.** None applicable; this lint reads source and emits diagnostics.

Quality method:

- `scrutineer` runs the gates and reports.
- Request a CodeRabbit review via `scrutineer` before marking the plan complete,
  and action every finding.

## Idempotence and recovery

Every step is re-runnable. `cargo` builds are incremental and safe to repeat.
UI-fixture blessing overwrites `.stderr` files in place, so review the diff
before committing; if a blessing goes wrong, `git checkout -- crates/string_continuation_style/ui/`
restores the committed expectations.

Commit after each milestone, and after each of the numbered edits in Step 5, so
that `git bisect` and time-travel review both work. Nothing in this plan writes
outside the repository except log files under `/tmp`, which are disposable.

If a milestone must be abandoned mid-flight, each milestone's Recovery entry
names the single revert that returns the tree to the previous plateau.

## Artefacts and notes

To be filled in as work proceeds. Required entries:

- `EV-M0-transcript` — the `EP-M0` spike output.
- The observed failure of each `NV-*` negative control.
- The `insta` snapshot of the RFC's motivating-example rewrite.
- The `make publish-check` tail showing `string_continuation_style` in
  `cargo dylint list`.

## Interfaces and dependencies

### Domain (pure; no `rustc_*` imports)

In `crates/string_continuation_style/src/facts.rs`, the input port — everything
the policy needs to know about a literal, with no compiler types:

```rust
/// What the adapter observed about one string literal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LiteralFacts<'a> {
    /// Raw source bytes between the quotes, exactly as written.
    pub body: &'a str,
    /// Which flavour of literal token this is.
    pub token: LiteralToken,
    /// Where the literal sits, and what the surrounding grammar permits.
    pub context: LiteralContext,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LiteralToken {
    CookedStr,
    RawStr,
    ByteStr,
    RawByteStr,
    CStr,
    RawCStr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LiteralContext {
    /// A general expression position in root syntax context.
    Expression,
    /// A source-authored format string in a proven macro contract.
    SourceFormatString { has_implicit_capture: bool },
    /// A format string produced by `concat!`, `include_str!`, or similar.
    GeneratedFormatString,
    /// The macro contract, format-string position, or span could not be proven.
    UnprovenContract,
    /// The editable source span could not be recovered exactly.
    UnrecoverableSpan,
}
```

In `crates/string_continuation_style/src/continuation/mod.rs`:

```rust
pub(crate) struct Continuation {
    pub escape_range: core::ops::Range<usize>,
    pub skipped_whitespace_range: core::ops::Range<usize>,
    pub kind: ContinuationKind,
}

pub(crate) enum ContinuationKind { Join, LeadingLayoutTrim, TrailingLayoutTrim }

/// Scans a cooked literal body for continuation escapes.
///
/// The caller must pass the body without its delimiting quotes.
pub(crate) fn scan(body: &str) -> Vec<Continuation>;
```

In `crates/string_continuation_style/src/classification/mod.rs`:

```rust
pub(crate) enum ContinuationDisposition {
    PreferConcat(ConcatRewrite),
    RequireContinuation(RequiredReason),
    Ignore,
}

pub(crate) enum RequiredReason {
    ImplicitFormatCapture,
    NonStringLiteralType,
    LeadingOrTrailingLayoutTrim,
    UnknownMacroContract,
    GeneratedFormatString,
    UnrecoverableSourceSpan,
}

/// The single policy entry point. Pure: same facts in, same disposition out.
pub(crate) fn classify(facts: &LiteralFacts<'_>) -> ContinuationDisposition;
```

In `crates/string_continuation_style/src/rewrite/mod.rs`:

```rust
pub(crate) struct ConcatRewrite {
    /// Source spellings of each fragment, in order, quotes excluded.
    pub fragments: Vec<core::ops::Range<usize>>,
    /// Byte offsets of each selected join's backslash, for secondary labels.
    pub join_offsets: Vec<usize>,
    pub applicability: RewriteApplicability,
}

pub(crate) enum RewriteApplicability { MachineApplicable, MaybeIncorrect }

/// Builds the replacement text for a whole literal.
pub(crate) fn render(body: &str, rewrite: &ConcatRewrite) -> String;
```

### Adapters (rustc-facing; `dylint-driver` feature only)

`crates/string_continuation_style/src/driver.rs` holds the `EarlyLintPass`
implementation, the `LiteralFacts` construction from `ExprKind::Lit` and
`ExprKind::FormatArgs`, the macro-identity allowlist, and the lint declaration
inside a private `declaration` module per the `conditional_max_n_branches`
pattern. It must stay under 400 lines; split the allowlist into
`src/driver/allowlist.rs` if it grows.

`crates/string_continuation_style/src/diagnostics.rs` converts a
`ContinuationDisposition` plus the recovered spans into a localized
`cx.emit_span_lint` call, following
`crates/bumpy_road_function/src/driver/diagnostic.rs`.

### Dependencies

The crate's `Cargo.toml` declares, all `optional = true` under `dylint-driver`:
`dylint_linting`, `rustc_ast`, `rustc_lint`, `rustc_session`, `rustc_span`,
`fluent-templates`, `log`, `serde`, `whitaker-common`, `whitaker` (with
`features = ["dylint-driver"]`). If `DEC-002` resolves to option 3, add
`rustc_hir` and `rustc_middle`. Do not add `clippy_utils`; no lint in this
repository uses it.

Dev-dependencies: `whitaker_test_macros`, `whitaker-common`, `whitaker`,
`rstest`, `rstest-bdd`, `rstest-bdd-macros`, `dylint_testing`, `proptest`,
`insta`, `googletest`, `pretty_assertions`, `serial_test`, `camino`,
`tempfile`, and — subject to `DEC-007` — `rustc-literal-escaper`.
